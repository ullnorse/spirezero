//! The interpreter: sts2-core's combat flow (`combat/turn.rs`, `card_play.rs`, `commands/*`,
//! `hooks.rs`) restricted to the supported content, over the `Copy` state.
//!
//! Each function names the sts2-core function it mirrors; the order of every step, guard and
//! hook pass is sts2-core's. Hooks no supported listener overrides are left out. Listener order
//! is sts2-core's (`CombatState::for_each_listener`): the player's powers, then each enemy's in
//! `Enemies` order, then the player's relics; async passes snapshot their listeners first and
//! re-check each one before its call (`Engine::dispatch`).

use sts2_core::content::cards::CardType;
use sts2_core::content::monsters::machine::Repeat;
use sts2_core::shuffle::{stable_shuffle_by, unstable_shuffle};

use crate::num::{Factors, scaled_trunc};
use crate::state::{
    Action, Fight, Forced, MAX_CARDS, MAX_CREATURES, MAX_HAND, MAX_POWERS, Monster, NONE, PLAYER,
    Power, Status, pile, with_rng,
};
use crate::tables::{Affliction, CardOp, Cond, Kernel, MoveOp, P, StateSpec, relic};

/// An effect met something the kernel does not support; the fight stops there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stop;

type Flow<T = ()> = Result<T, Stop>;

/// `ValueProp` flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Props(u8);

impl Props {
    const UNBLOCKABLE_UNPOWERED_MOVE: Self = Self(7);
    const UNPOWERED: Self = Self(2);
    const MOVE: Self = Self(4);
    const UNPOWERED_MOVE: Self = Self(6);

    /// `Unblockable`.
    const fn unblockable(self) -> bool {
        self.0 & 1 != 0
    }

    /// `IsPoweredAttack` / `IsPoweredCardOrMonsterMoveBlock`: `Move` and not `Unpowered`.
    const fn powered(self) -> bool {
        self.0 & 4 != 0 && self.0 & 2 == 0
    }
}

/// `CardPile.MaxCardsInHand`, as an `i32` draw bound.
const HAND_DRAW: i32 = 5;
/// The power kinds with an `AfterSideTurnEnd` override.
const SIDE_TURN_END: u32 = P::Vulnerable.bit()
    | P::Weak.bit()
    | P::Frail.bit()
    | P::Territorial.bit()
    | P::Ringing.bit()
    | P::Shrink.bit()
    | P::Constrict.bit()
    | P::Tangled.bit()
    | P::Ritual.bit()
    | P::Skittish.bit()
    | P::Asleep.bit()
    | P::Intangible.bit();
/// `AfterSideTurnStart` overrides.
const SIDE_TURN_START: u32 = P::Slow.bit() | P::Plating.bit();
/// `BeforeSideTurnStart` and `BeforeSideTurnEndEarly` overrides.
const PLATING: u32 = P::Plating.bit();
/// `AfterDeath` overrides.
const AFTER_DEATH: u32 = P::Shrink.bit()
    | P::Infested.bit()
    | P::Constrict.bit()
    | P::Illusion.bit()
    | P::Ravenous.bit();
/// `AfterDamageReceived` overrides.
const AFTER_DAMAGE_RECEIVED: u32 =
    P::Plow.bit() | P::Slippery.bit() | P::Shriek.bit() | P::Asleep.bit() | P::HardenedShell.bit();
/// `BeforeDamageReceived` overrides.
const THORNS: u32 = P::Thorns.bit();
/// `AfterAttack` overrides.
const AFTER_ATTACK: u32 = P::Suck.bit() | P::Vigor.bit() | P::Skittish.bit();
/// `AfterCardEnteredCombat` overrides.
const AFTER_CARD_ENTERED: u32 = P::Ringing.bit() | P::Tangled.bit();
/// `Wriggler` slots of `PhrogParasiteElite` (`InfestedPower`).
const WRIGGLER_SLOTS: [&str; 4] = ["wriggler1", "wriggler2", "wriggler3", "wriggler4"];
/// The kernel monster index of `Wriggler` (`tables::supported_monsters`).
const WRIGGLER: u8 = 1;

/// One listener of an async pass: the instance as it was when the pass started.
#[derive(Clone, Copy, Debug, Default)]
struct Entry {
    owner: u8,
    power: Power,
}

/// Listeners per async pass: no supported hook has more than a few (two per creature at most).
const MAX_PASS: usize = 16;

/// The listeners of one async pass.
struct Snapshot {
    entries: [Entry; MAX_PASS],
    len: usize,
}

/// A hit's result (`DamageResult`) as far as supported listeners read it.
#[derive(Clone, Copy, Debug)]
struct Hit {
    /// HP actually lost.
    unblocked: i32,
    killed: bool,
    /// `WasFullyBlocked`.
    fully_blocked: bool,
}

impl Fight {
    pub(crate) fn alive(&self, id: u8) -> bool {
        self.creature(id).alive()
    }

    pub(crate) fn player_dead(&self) -> bool {
        !self.alive(PLAYER)
    }

    /// `CombatManager.IsEnding`: a pending loss, or no living primary enemy and no
    /// `ShouldStopCombatFromEnding` (`InfestedPower`) listener.
    pub(crate) fn is_ending(&self) -> bool {
        let f = self;
        if !f.in_progress {
            return false;
        }
        if f.pending_loss {
            return true;
        }
        let mut infested = false;
        for &e in f.enemies() {
            let c = f.creature(e);
            if c.alive() && c.primary_enemy() {
                return false;
            }
            infested |= c.has(P::Infested);
        }
        !infested
    }

    /// `IsOverOrEnding`; also whether a `Combat`-guarded hook pass is closed.
    pub(crate) fn over_or_ending(&self) -> bool {
        !self.in_progress || self.is_ending()
    }

    /// `CombatState::contains_listener` for a power of `owner`.
    pub(crate) fn listens(&self, owner: u8) -> bool {
        self.contains(owner) && (owner != PLAYER || self.hooks_active)
    }

    /// The listeners, in order, among the powers of kinds in `mask`.
    fn snapshot(&self, mask: u32) -> Flow<Snapshot> {
        let mut snap = Snapshot {
            entries: [Entry::default(); MAX_PASS],
            len: 0,
        };
        let mut full = false;
        let f = self;
        let mut push = |owner: u8| {
            let c = f.creature(owner);
            if c.mask & mask == 0 {
                return;
            }
            for &power in c.powers() {
                if power.kind.bit() & mask != 0 {
                    if snap.len == MAX_PASS {
                        full = true;
                        return;
                    }
                    snap.entries[snap.len] = Entry { owner, power };
                    snap.len += 1;
                }
            }
        };
        push(PLAYER);
        for &e in f.enemies() {
            push(e);
        }
        if full { Err(Stop) } else { Ok(snap) }
    }

    /// Whether any creature in combat has a power of a kind in `mask`.
    fn any_listener(&self, mask: u32) -> bool {
        self.player().mask & mask != 0
            || self
                .enemies()
                .iter()
                .any(|&e| self.creature(e).mask & mask != 0)
    }

    /// The instance of `entry` now: still on its owner, or as it was when it was removed (the
    /// snapshot's copy; a removed instance keeps running, `CombatState::power`).
    fn instance(&self, entry: &Entry) -> Power {
        let c = self.creature(entry.owner);
        c.by_uid(entry.power.uid)
            .map_or(entry.power, |i| c.powers[i])
    }

    /// `Hook.ShouldAllowHitting` (guarded): no reviving `IllusionPower` owner.
    pub(crate) fn allows_hitting(&self, id: u8) -> bool {
        if self.over_or_ending() || !self.contains(id) {
            return true;
        }
        let c = self.creature(id);
        c.find(P::Illusion).is_none_or(|i| c.powers[i].data == 0)
    }

    /// `Creature.CanReceivePowers`.
    pub(crate) fn can_receive(&self, id: u8) -> bool {
        let c = self.creature(id);
        let attached = !c.enemy || self.contains(id) || c.alive();
        attached && self.allows_hitting(id)
    }

    /// `CardEnergyCost.GetWithModifiers(All)`: the base, then `Hook.ModifyEnergyCostInCombat`
    /// (`TangledPower` on an Entangled card), then `max(0, x)`; a negative base as is.
    pub(crate) fn energy_cost(&self, k: &Kernel, card: u8) -> i32 {
        let c = self.card(card);
        let cost = k.cards[usize::from(c.spec)].cost;
        if cost < 0 {
            return cost;
        }
        let player = self.player();
        if c.affliction == Affliction::Entangled && player.has(P::Tangled) && !self.over_or_ending()
        {
            return (cost + player.amount(P::Tangled)).max(0);
        }
        cost
    }

    /// `Hook.ShouldPlay` (guarded): `RingingPower` vetoes a Ringing card once a card play started
    /// this turn.
    pub(crate) fn should_play(&self, card: u8) -> bool {
        !(self.card(card).affliction == Affliction::Ringing
            && self.played_this_turn
            && self.player().has(P::Ringing)
            && !self.over_or_ending())
    }

    /// `CanPlay` and `IsValidTarget` for a manual play of a hand card.
    pub(crate) fn can_play(&self, k: &Kernel, card: u8, target: u8) -> bool {
        let spec = &k.cards[usize::from(self.card(card).spec)];
        if spec.unplayable
            || self.energy < self.energy_cost(k, card).max(0)
            || !self.should_play(card)
        {
            return false;
        }
        if spec.targeted {
            target != NONE && self.enemies().contains(&target) && self.alive(target)
        } else {
            target == NONE
        }
    }

    /// `Hook.ModifyDamage`, truncated, and whether the exact amount is positive: the dealer's
    /// Strength and Vigor, then the target's Vulnerable and Slow and the dealer's Weak and Shrink
    /// (exact, so their order does not matter), then the target's Intangible cap.
    fn modify_damage(
        &self,
        target: u8,
        dealer: u8,
        props: Props,
        amount: i32,
    ) -> Flow<(i32, bool)> {
        let mut base = i64::from(amount);
        let mut factors = Factors::new();
        if props.powered() && dealer != NONE {
            let d = self.creature(dealer);
            let t = self.creature(target);
            base += i64::from(d.amount(P::Strength)) + i64::from(d.amount(P::Vigor));
            if t.mask & (P::Vulnerable.bit() | P::Slow.bit()) != 0 {
                for p in t.powers() {
                    match p.kind {
                        P::Vulnerable => {
                            factors.push(150);
                        }
                        P::Slow => {
                            let n = u32::try_from(p.data).map_err(|_| Stop)?;
                            factors.push(100 + 10 * n);
                        }
                        _ => {}
                    }
                }
            }
            if d.mask & (P::Weak.bit() | P::Shrink.bit()) != 0 {
                for p in d.powers() {
                    match p.kind {
                        P::Weak => {
                            factors.push(75);
                        }
                        P::Shrink => {
                            factors.push(70);
                        }
                        _ => {}
                    }
                }
            }
        }
        let mut damage = scaled_trunc(base, &factors).ok_or(Stop)?;
        // ModifyDamageCap (unguarded): IntangiblePower caps the owner's damage at 1.
        if self.creature(target).has(P::Intangible) {
            damage = damage.min(1);
        }
        Ok((damage, base > 0))
    }

    /// `Hook.ModifyPowerAmountReceived` (guarded): the target's `ArtifactPower` negates a debuff.
    /// Returns the amount and the Artifact's uid if it did.
    pub(crate) fn modify_received(
        &self,
        k: &Kernel,
        target: u8,
        p: P,
        amount: i32,
    ) -> (i32, Option<u16>) {
        if self.over_or_ending() || !self.contains(target) {
            return (amount, None);
        }
        let c = self.creature(target);
        match c.find(P::Artifact) {
            Some(i) if k.debuff_amount(p, amount) => (0, Some(c.powers[i].uid)),
            _ => (amount, None),
        }
    }
}

/// The state and tables of one step.
pub struct Eng<'a> {
    pub f: &'a mut Fight,
    pub k: &'a Kernel,
}

impl Eng<'_> {
    // ----- steps -----------------------------------------------------------------------------

    /// One player action; the caller has checked it.
    pub fn act(&mut self, action: Action) -> Flow {
        match action {
            Action::Play { card, target } => self.play_card_action(card, target),
            Action::EndTurn => self.end_player_turn(),
            Action::Pick(card) => self.resolve(card),
        }
    }

    /// `PlayCardAction`: spend the energy, `OnPlayWrapper`, then `CheckWinCondition`.
    fn play_card_action(&mut self, card: u8, target: u8) -> Flow {
        let cost = self.f.energy_cost(self.k, card).max(0);
        if cost > 0 {
            self.f.energy -= cost;
        }
        // AddDuringManualCardPlay: hand to the Play pile.
        if !self.f.over_or_ending() {
            self.f.remove_from_pile(card);
            self.f.insert_at_bottom(card, pile::PLAY);
        }
        if self.f.player_dead() {
            self.check_win_condition();
            return Ok(());
        }
        if self.f.over_or_ending() {
            // The play loop does not start; the card goes to its result pile.
            self.move_to_result_pile(card);
            self.check_win_condition();
            return Ok(());
        }
        self.f.played_this_turn = true;
        self.continue_play(card, target, 0)
    }

    /// The card's `OnPlay` from op `from` on, then the rest of `OnPlayWrapper` and the
    /// `CheckWinCondition`; stops early at a pending choice.
    fn continue_play(&mut self, card: u8, target: u8, from: usize) -> Flow {
        let k = self.k;
        let spec = &k.cards[usize::from(self.f.card(card).spec)];
        for &op in &spec.ops[from..] {
            match op {
                CardOp::Attack(damage) => self.attack(PLAYER, target, damage, 1, true)?,
                CardOp::Block(block) => self.gain_block(PLAYER, block, Props::MOVE),
                CardOp::ApplyTarget(p, amount) => self.apply_power(target, p, amount, PLAYER)?,
                CardOp::Draw(count) => self.draw(count),
                CardOp::DiscardOne => {
                    // CardSelectCmd.FromHandForDiscard(.., 1): no prompt while ending, for an
                    // empty hand, or when one card leaves no choice.
                    if self.f.over_or_ending() || self.f.hand().is_empty() {
                        continue;
                    }
                    if self.f.hand().len() == 1 {
                        let only = self.f.hand()[0];
                        self.discard(only);
                        continue;
                    }
                    self.f.pending = card;
                    return Ok(());
                }
            }
        }
        if !self.f.player_dead() {
            if self.f.in_progress {
                self.after_card_played();
            }
            if !self.f.player_dead() {
                self.move_to_result_pile(card);
            }
        }
        {
            self.check_win_condition();
            Ok(())
        }
    }

    /// The answer to the pending discard: discard it, then finish the play.
    fn resolve(&mut self, pick: u8) -> Flow {
        let card = self.f.pending;
        self.f.pending = NONE;
        self.discard(pick);
        let spec = &self.k.cards[usize::from(self.f.card(card).spec)];
        let next = spec
            .ops
            .iter()
            .position(|&op| op == CardOp::DiscardOne)
            .expect("the pending card discards")
            + 1;
        self.continue_play(card, NONE, next)
    }

    /// The end of `OnPlayWrapper`: a card still in the Play pile goes to its result pile.
    fn move_to_result_pile(&mut self, card: u8) {
        if self.f.card(card).pile != pile::PLAY {
            return;
        }
        if self.k.cards[usize::from(self.f.card(card).spec)].exhaust {
            self.exhaust(card);
        } else {
            self.add_to_pile(card, pile::DISCARD);
        }
    }

    /// `EndPlayerTurnAction` up to the next player play phase or combat end.
    fn end_player_turn(&mut self) -> Flow {
        // Phase one: BeforeSideTurnEnd, DoTurnEnd, BeforeFlush, each followed by the check.
        self.before_side_turn_end(&[PLAYER])?;
        if self.check_win_condition() {
            return Ok(());
        }
        self.do_turn_end()?;
        if self.check_win_condition() {
            return Ok(());
        }
        if self.check_win_condition() || !self.f.in_progress {
            return Ok(());
        }
        // Phase two.
        self.flush_hand();
        self.after_side_turn_end(false, &[PLAYER])?;
        // SwitchFromPlayerToEnemySide.
        self.switch_sides();
        self.start_turn()?;
        self.start_turn()
    }

    /// `DoTurnEnd`: exhaust the Ethereal hand cards, then each turn-end-in-hand card in hand
    /// order.
    fn do_turn_end(&mut self) -> Flow {
        if !self.f.in_progress || self.f.is_ending() {
            return Ok(());
        }
        let k = self.k;
        let mut ethereal = [0; MAX_HAND];
        let mut n_ethereal = 0;
        let mut turn_end = [0; MAX_HAND];
        let mut n_turn_end = 0;
        for &card in self.f.hand() {
            let spec = &k.cards[usize::from(self.f.card(card).spec)];
            if spec.turn_end.is_some() {
                turn_end[n_turn_end] = card;
                n_turn_end += 1;
            } else if spec.ethereal {
                ethereal[n_ethereal] = card;
                n_ethereal += 1;
            }
        }
        for &card in &ethereal[..n_ethereal] {
            self.exhaust(card);
        }
        for &card in &turn_end[..n_turn_end] {
            self.add_to_pile(card, pile::PLAY);
            let spec = &k.cards[usize::from(self.f.card(card).spec)];
            let turn_end = spec.turn_end.expect("a turn-end card");
            if turn_end.unblockable {
                self.damage(
                    PLAYER,
                    turn_end.amount,
                    Props::UNBLOCKABLE_UNPOWERED_MOVE,
                    NONE,
                )?;
            } else {
                self.damage(PLAYER, turn_end.amount, Props::UNPOWERED_MOVE, PLAYER)?;
            }
            if spec.ethereal {
                self.exhaust(card);
            } else {
                self.add_to_pile(card, pile::DISCARD);
            }
        }
        Ok(())
    }

    /// `FlushPlayerHand`: every hand card to the discard pile, as one batch.
    fn flush_hand(&mut self) {
        if self.f.player_dead() {
            return;
        }
        let mut cards = [0; MAX_HAND];
        let hand = self.f.hand();
        cards[..hand.len()].copy_from_slice(hand);
        let n = hand.len();
        self.add_cards(&cards[..n], pile::DISCARD);
    }

    /// `SwitchSides`.
    fn switch_sides(&mut self) {
        if self.f.enemy_side {
            self.f.enemy_side = false;
            self.f.round += 1;
            self.f.turn += 1;
        } else {
            self.f.enemy_side = true;
        }
        // The turn's history key changes.
        self.f.played_this_turn = false;
        // Creature.OnSideSwitch, for the creatures in combat.
        for i in 0..usize::from(self.f.n_enemies) {
            let e = self.f.enemies[i];
            self.f.creature_mut(e).monster.spawned_this_turn = false;
        }
    }

    /// `StartTurn` for the current side.
    fn start_turn(&mut self) -> Flow {
        if !self.f.in_progress {
            return Ok(());
        }
        let enemy = self.f.enemy_side;
        let mut creatures = [0; MAX_CREATURES];
        let n = if enemy {
            let e = self.f.enemies();
            creatures[..e.len()].copy_from_slice(e);
            e.len()
        } else {
            creatures[0] = PLAYER;
            1
        };
        let creatures = &creatures[..n];
        self.before_side_turn_start(enemy)?;
        if !enemy {
            let mut list = [0; MAX_CREATURES];
            let e = self.f.enemies();
            list[..e.len()].copy_from_slice(e);
            for &id in &list[..e.len()] {
                self.roll_move(id)?;
            }
        }
        for &id in creatures {
            // Creature.AfterTurnStart: ClearBlock (not on the player's first turn).
            if !(id == PLAYER && self.f.turn == 1) {
                self.f.creature_mut(id).block = 0;
            }
        }
        if !enemy {
            self.setup_player_turn()?;
        }
        self.after_side_turn_start(creatures)?;
        if enemy {
            if self.check_win_condition() {
                return Ok(());
            }
            self.execute_enemy_turn()
        } else {
            {
                self.check_win_condition();
                Ok(())
            }
        }
    }

    /// `SetupPlayerTurn`: reset energy, draw the hand.
    fn setup_player_turn(&mut self) -> Flow {
        if self.f.player_dead() {
            return Ok(());
        }
        self.f.energy = self.f.max_energy;
        let mut hand_draw = HAND_DRAW;
        if !self.f.over_or_ending()
            && self.f.relics & relic::RING_OF_THE_SNAKE != 0
            && self.f.turn <= 1
        {
            hand_draw += 2;
        }
        if self.f.turn == 1 {
            // Innate and enchantment ordering of the first draw: sts2-core plays turn 1.
            return Err(Stop);
        }
        self.draw(hand_draw);
        Ok(())
    }

    /// `ExecuteEnemyTurn`, then `EndEnemyTurn`.
    fn execute_enemy_turn(&mut self) -> Flow {
        let mut list = [0; MAX_CREATURES];
        let e = self.f.enemies();
        let n = e.len();
        list[..n].copy_from_slice(e);
        for &id in &list[..n] {
            if !self.f.contains(id) {
                continue;
            }
            if !self.f.creature(id).monster.spawned_this_turn {
                self.perform_move(id)?;
            }
            self.check_win_condition();
            if !self.f.in_progress {
                return Ok(());
            }
        }
        let e = self.f.enemies();
        let n = e.len();
        list[..n].copy_from_slice(e);
        self.before_side_turn_end(&list[..n])?;
        self.after_side_turn_end(true, &list[..n])?;
        self.check_win_condition();
        if !self.f.is_ending() {
            self.switch_sides();
        }
        Ok(())
    }

    /// `CheckWinCondition`. Returns whether combat ended here.
    fn check_win_condition(&mut self) -> bool {
        if self.f.pending_loss {
            self.f.in_progress = false;
            self.f.status = Status::Lost;
            return true;
        }
        if self.f.is_ending() {
            self.f.in_progress = false;
            self.f.status = Status::Won;
            // CombatHistory.Clear.
            self.f.played_this_turn = false;
            // Player.AfterCombatEnd: the piles are cleared, the powers removed without
            // callbacks, the block lost.
            for c in &mut self.f.cards[..usize::from(self.f.n_cards)] {
                c.pile = NONE;
            }
            self.f.hand.clear();
            self.f.play.clear();
            self.f.draw.clear();
            self.f.discard.clear();
            self.f.exhaust.clear();
            let p = self.f.creature_mut(PLAYER);
            p.n_powers = 0;
            p.mask = 0;
            p.block = 0;
            // Hook.AfterCombatVictory: BurningBlood.
            if self.f.relics & relic::BURNING_BLOOD != 0
                && self.f.hooks_active
                && !self.f.player_dead()
            {
                self.heal(PLAYER, 6);
            }
            return true;
        }
        false
    }

    // ----- cards and piles -------------------------------------------------------------------

    /// `CardPileCmd.Add(card, pile, Bottom)`.
    fn add_to_pile(&mut self, card: u8, p: u8) {
        self.add_cards(&[card], p);
    }

    /// `CardPileCmd.Add(cards, pile, Bottom)`: one batch; a full hand redirects to the discard
    /// pile; `AfterCardEnteredCombat` for a card that had no pile.
    fn add_cards(&mut self, cards: &[u8], p: u8) {
        if cards.is_empty() || self.f.is_ending() || self.f.player_dead() || !self.f.in_progress {
            return;
        }
        for &card in cards {
            let target = if p == pile::HAND && self.f.hand().len() >= MAX_HAND {
                pile::DISCARD
            } else {
                p
            };
            let old = self.f.remove_from_pile(card);
            self.f.insert_at_bottom(card, target);
            if old.is_none() {
                self.after_card_entered_combat(card);
            }
        }
    }

    /// `CardCmd.Exhaust`.
    fn exhaust(&mut self, card: u8) {
        if self.f.over_or_ending() {
            return;
        }
        self.add_to_pile(card, pile::EXHAUST);
    }

    /// `CardCmd.Discard` of one card.
    fn discard(&mut self, card: u8) {
        if self.f.over_or_ending() {
            return;
        }
        self.add_to_pile(card, pile::DISCARD);
    }

    /// `CardPileCmd.Draw(count)`. Nothing a draw does changes a creature or puts a card into
    /// combat (a drawn card already had a pile, and a shuffle only moves cards between piles), so
    /// the guards sts2-core re-checks per card (`IsOverOrEnding`, `CardPileCmd.Add`'s) keep the
    /// value they had at the start; the loop checks only the piles.
    fn draw(&mut self, count: i32) {
        if self.f.over_or_ending() {
            return;
        }
        for _ in 0..count.max(0) {
            if self.f.hand.len() >= MAX_HAND {
                break;
            }
            if self.f.draw.is_empty() {
                if self.f.discard.is_empty() {
                    break;
                }
                self.shuffle();
            }
            let Some(card) = self.f.draw.pop() else {
                break;
            };
            self.f.hand.push(card);
            self.f.cards[usize::from(card)].pile = pile::HAND;
        }
    }

    /// `CardPileCmd.Shuffle`: discard ++ draw, `StableShuffle` on `Rng.Shuffle` (sorted by
    /// `CardModel.CompareTo`), then one batch to the draw pile's bottom. Every card of the batch
    /// already sits in the discard or draw pile, so the batch leaves the shuffled list as the
    /// draw pile and the discard pile empty.
    fn shuffle(&mut self) {
        if self.f.over_or_ending() {
            return;
        }
        let k = self.k;
        let mut list = [(0_u16, 0_u8); MAX_CARDS];
        let mut n = 0;
        for card in self
            .f
            .pile_cards(pile::DISCARD)
            .chain(self.f.pile_cards(pile::DRAW))
        {
            let rank = k.cards[usize::from(self.f.card(card).spec)].rank;
            list[n] = (rank, card);
            n += 1;
        }
        let list = &mut list[..n];
        if n <= 16 && n != 3 {
            // .NET's sort of up to 16 items is an insertion sort (stable) except for exactly
            // three items (`SwapIfGreater` three times, which is not), so here it equals a
            // stable sort by rank.
            stable_sort_by_rank(list);
            with_rng(&mut self.f.rng_shuffle, |rng| unstable_shuffle(list, rng));
        } else {
            with_rng(&mut self.f.rng_shuffle, |rng| {
                stable_shuffle_by(list, rng, |a, b| a.0.cmp(&b.0));
            });
        }
        self.f.discard.clear();
        self.f.draw.clear();
        for &(_, card) in list.iter().rev() {
            self.f.draw.push(card);
            self.f.cards[usize::from(card)].pile = pile::DRAW;
        }
    }

    /// `CardCmd.Afflict(card, kind, 1)` for a non-stacking affliction.
    fn afflict(&mut self, card: u8, kind: Affliction) {
        if self.f.over_or_ending() && self.f.card(card).pile != NONE {
            return;
        }
        let c = &mut self.f.cards[usize::from(card)];
        if c.affliction == Affliction::None {
            c.affliction = kind;
        }
    }

    /// The cards of every combat pile, in pile order (`PlayerCombatState.AllCards`).
    fn all_pile_cards(&self) -> ([u8; MAX_CARDS], usize) {
        let mut out = [0; MAX_CARDS];
        let mut n = 0;
        for p in 0..5 {
            for card in self.f.pile_cards(p) {
                out[n] = card;
                n += 1;
            }
        }
        (out, n)
    }

    /// `CombatState.CreateCard` plus `AddGeneratedCardToCombat` to the discard pile, `count`
    /// times (`CardPileCmd.AddToCombatAndPreview<T>(PlayerCreatures, Discard, count)`); with
    /// `random_draw`, into the draw pile at `Rng.Shuffle.NextInt(count + 1)` from the top. A
    /// `direct` card (`CombatState.CreateCard`) is created even for a dead player.
    fn add_status_cards(&mut self, spec: u8, count: u8, direct: bool, random_draw: bool) -> Flow {
        if self.f.player_dead() && !direct {
            return Ok(());
        }
        for _ in 0..count {
            let id = self.f.n_cards;
            if usize::from(id) >= MAX_CARDS {
                return Err(Stop);
            }
            self.f.cards[usize::from(id)] = crate::state::Card {
                spec,
                pile: NONE,
                affliction: Affliction::None,
            };
            self.f.n_cards += 1;
            if !self.f.in_progress {
                continue;
            }
            if random_draw {
                if self.f.is_ending() || self.f.player_dead() {
                    continue;
                }
                let len = self.f.draw.len();
                let bound = i32::try_from(len + 1).expect("pile size fits i32");
                let from_top = with_rng(&mut self.f.rng_shuffle, |rng| rng.next_int(bound));
                let from_top = usize::try_from(from_top).expect("NextInt is non-negative");
                self.f.draw.insert(len - from_top, id);
                self.f.cards[usize::from(id)].pile = pile::DRAW;
                self.after_card_entered_combat(id);
            } else {
                self.add_to_pile(id, pile::DISCARD);
            }
        }
        Ok(())
    }

    // ----- damage and block ------------------------------------------------------------------

    /// `AttackCommand.Execute` of a single-target attack (`target`, or the players for
    /// [`NONE`]), `hits` powered hits of `damage`; `from_card` for a card's attack.
    fn attack(
        &mut self,
        attacker: u8,
        target: u8,
        damage: i32,
        hits: i32,
        from_card: bool,
    ) -> Flow {
        if self.f.over_or_ending() || !self.f.alive(attacker) {
            return Ok(());
        }
        let vigor = self.before_attack(attacker);
        let target = if target == NONE { PLAYER } else { target };
        let mut took_hp = 0;
        let mut first = None;
        for _ in 0..hits {
            if !self.f.alive(attacker) || !self.f.alive(target) {
                break;
            }
            let lost = self.damage(target, damage, Props::MOVE, attacker)?;
            first.get_or_insert(lost);
            took_hp += i32::from(lost > 0);
        }
        self.after_attack(attacker, target, from_card, took_hp, first, vigor)
    }

    /// `CreatureCmd.Damage` on one target (`dealer` [`NONE`] for none). Returns the HP it lost.
    fn damage(&mut self, target: u8, amount: i32, props: Props, dealer: u8) -> Flow<i32> {
        if (dealer != NONE && !self.f.alive(dealer)) || !self.f.alive(target) {
            return Ok(0);
        }
        let hit = self.damage_target(target, amount, props, dealer)?;
        if hit.killed && !self.f.alive(target) {
            self.kill(&[target])?;
        } else {
            self.after_damage_received(target, hit)?;
        }
        Ok(hit.unblocked)
    }

    /// One target of `CreatureCmd.Damage`: `ModifyDamage`, `BeforeDamageReceived`, block,
    /// `ModifyHpLost`, HP loss. Every step after `ModifyDamage` reads only the truncated amount,
    /// and whether the exact one is positive (`num.rs`).
    fn damage_target(&mut self, target: u8, amount: i32, props: Props, dealer: u8) -> Flow<Hit> {
        let (modified, positive) = self.f.modify_damage(target, dealer, props, amount)?;
        self.before_damage_received(target, dealer, props)?;
        let in_progress = self.f.in_progress;
        let unblockable = props.unblockable();
        let c = self.f.creature_mut(target);
        let block_before = c.block;
        let blocked = if unblockable {
            0
        } else {
            c.block.min(modified)
        };
        c.block -= blocked;
        let mut unblocked = modified - blocked;
        // ModifyHpLost(BeforeOsty), Late pass: HardenedShellPower caps the turn's loss.
        if unblocked != 0
            && let Some(i) = c.find(P::HardenedShell)
        {
            unblocked = unblocked.min(c.powers[i].amount - c.powers[i].data);
        }
        // ModifyHpLost(AfterOsty): SlipperyPower, and IntangiblePower in a live combat, cap the
        // owner's loss at 1.
        if unblocked >= 1 && (c.has(P::Slippery) || (in_progress && c.has(P::Intangible))) {
            unblocked = 1;
        }
        let unblocked = unblocked.max(0);
        let before = c.hp;
        let killed = before > 0 && unblocked >= before;
        c.hp = (before - unblocked.min(999_999_999)).max(0);
        let fully_blocked =
            !unblockable && ((block_before > 0 && positive) || c.block > 0) && unblocked == 0;
        Ok(Hit {
            unblocked: before - c.hp,
            killed,
            fully_blocked,
        })
    }

    /// `CreatureCmd.GainBlock`: `Hook.ModifyBlock` (`DexterityPower`, then `FrailPower`),
    /// truncated.
    fn gain_block(&mut self, id: u8, amount: i32, props: Props) {
        if self.f.over_or_ending() || !self.f.alive(id) {
            return;
        }
        let c = self.f.creature_mut(id);
        let mut base = i64::from(amount);
        let mut factors = Factors::new();
        if props.powered() {
            base += i64::from(c.amount(P::Dexterity));
            if c.has(P::Frail) {
                factors.push(75);
            }
        }
        // A base this small never leaves the exact domain.
        let modified = scaled_trunc(base, &factors).unwrap_or(0);
        if modified > 0 {
            c.block = (c.block + modified).min(999_999_999);
        }
    }

    /// `CreatureCmd.Heal`.
    fn heal(&mut self, id: u8, amount: i32) {
        if self.f.is_ending() && id != PLAYER {
            return;
        }
        let c = self.f.creature_mut(id);
        c.hp = (c.hp + amount).min(c.max_hp);
    }

    /// `CreatureCmd.Kill(creatures)`.
    fn kill(&mut self, ids: &[u8]) -> Flow {
        if ids.is_empty() {
            return Ok(());
        }
        for &id in ids {
            self.kill_one(id)?;
        }
        if ids.contains(&PLAYER) && self.f.player_dead() && self.f.in_progress {
            self.f.pending_loss = true;
        }
        Ok(())
    }

    /// `KillWithoutCheckingWinCondition` (no supported death preventer).
    fn kill_one(&mut self, id: u8) -> Flow {
        self.f.creature_mut(id).hp = 0;
        // Hook.ShouldCreatureBeRemovedFromCombatAfterDeath: a creature's own IllusionPower vetoes.
        let remove = !(self.f.contains(id) && self.f.creature(id).has(P::Illusion));
        self.after_death(id)?;
        let enemy = self.f.creature(id).enemy;
        let mut teammates = [0; MAX_CREATURES];
        let mut n_teammates = 0;
        if enemy {
            for &e in self.f.enemies() {
                if self.f.alive(e) {
                    teammates[n_teammates] = e;
                    n_teammates += 1;
                }
            }
        }
        if remove
            && enemy
            && self.f.enemies().contains(&id)
            && !self.f.creature(id).monster.performing
        {
            self.remove_enemy(id);
        }
        let primary = self.f.creature(id).primary_enemy();
        // RemoveAllPowersAfterDeath: with an IllusionPower listening, only the debuffs go.
        let illusion = self
            .f
            .enemies()
            .iter()
            .any(|&e| self.f.creature(e).has(P::Illusion));
        let k = self.k;
        let c = self.f.creature_mut(id);
        let mut kept = 0;
        let mut removed = [Power::default(); MAX_POWERS];
        let mut n_removed = 0;
        for i in 0..usize::from(c.n_powers) {
            let p = c.powers[i];
            let info = k.power(p.kind);
            let goes = info.removed_on_owner_death && (!illusion || info.debuff && !info.temporary);
            if goes {
                removed[n_removed] = p;
                n_removed += 1;
            } else {
                c.powers[kept] = p;
                kept += 1;
            }
        }
        c.n_powers = u8::try_from(kept).expect("few powers");
        c.remask();
        for p in &removed[..n_removed] {
            self.after_removed(p.kind);
        }
        if enemy {
            if primary
                && n_teammates > 0
                && teammates[..n_teammates]
                    .iter()
                    .all(|&t| self.f.creature(t).secondary())
            {
                self.kill(&teammates[..n_teammates])?;
            }
            return Ok(());
        }
        self.f.hooks_active = false;
        Ok(())
    }

    fn remove_enemy(&mut self, id: u8) {
        let n = usize::from(self.f.n_enemies);
        if let Some(i) = self.f.enemies[..n].iter().position(|&e| e == id) {
            self.f.enemies.copy_within(i + 1..n, i);
            self.f.n_enemies -= 1;
        }
    }

    // ----- powers ----------------------------------------------------------------------------

    /// `PowerCmd.Apply<T>(target, amount, applier)`.
    fn apply_power(&mut self, target: u8, p: P, amount: i32, applier: u8) -> Flow {
        if self.f.is_ending() || amount == 0 || !self.f.can_receive(target) {
            return Ok(());
        }
        if let Some(i) = self.f.creature(target).find(p) {
            let uid = self.f.creature(target).powers[i].uid;
            return self.modify_amount(target, uid, amount);
        }
        let (modified, artifact) = self.f.modify_received(self.k, target, p, amount);
        if !self.f.can_receive(target) {
            return Ok(());
        }
        let added = if modified == 0 {
            false
        } else {
            self.add_power(target, p, modified, applier)?;
            true
        };
        if let Some(uid) = artifact {
            self.after_modifying_received(target, uid)?;
        }
        if added {
            self.after_applied(target, p)?;
        }
        Ok(())
    }

    /// `Hook.AfterModifyingPowerAmountReceived` (guarded) for the Artifact that negated a debuff:
    /// it loses a stack.
    fn after_modifying_received(&mut self, owner: u8, uid: u16) -> Flow {
        if self.f.over_or_ending() || !self.f.listens(owner) {
            return Ok(());
        }
        self.modify_amount(owner, uid, -1)
    }

    /// `ApplyInternal`: a new instance (a player's debuff skips its next duration tick).
    fn add_power(&mut self, target: u8, p: P, amount: i32, applier: u8) -> Flow {
        let uid = self.f.next_uid;
        self.f.next_uid += 1;
        let debuff = self.k.power(p).debuff;
        let c = self.f.creature_mut(target);
        let n = usize::from(c.n_powers);
        if n == MAX_POWERS {
            return Err(Stop);
        }
        c.powers[n] = Power {
            kind: p,
            skip_tick: target == PLAYER && debuff,
            applier,
            uid,
            amount: amount.clamp(-999_999_999, 999_999_999),
            data: 0,
        };
        c.n_powers += 1;
        c.mask |= p.bit();
        Ok(())
    }

    /// `PowerCmd.ModifyAmount(power, offset)`, then the removal its amount calls for.
    fn modify_amount(&mut self, owner: u8, uid: u16, offset: i32) -> Flow {
        if self.f.is_ending() || !self.f.contains(owner) {
            return Ok(());
        }
        let Some(i) = self.f.creature(owner).by_uid(uid) else {
            // A detached instance: sts2-core keeps running it; no supported content gets here.
            return Err(Stop);
        };
        let kind = self.f.creature(owner).powers[i].kind;
        let (modified, artifact) = self.f.modify_received(self.k, owner, kind, offset);
        let p = &mut self.f.creature_mut(owner).powers[i];
        p.amount = (p.amount + modified).clamp(-999_999_999, 999_999_999);
        if let Some(a) = artifact {
            self.after_modifying_received(owner, a)?;
        }
        let c = self.f.creature(owner);
        let Some(i) = c.by_uid(uid) else {
            return Ok(());
        };
        let amount = c.powers[i].amount;
        let remove = if self.k.power(kind).allows_negative {
            amount == 0
        } else {
            amount <= 0
        };
        if remove {
            self.remove_power(owner, uid);
        }
        Ok(())
    }

    /// `PowerCmd.Decrement`.
    fn decrement(&mut self, owner: u8, uid: u16) -> Flow {
        self.modify_amount(owner, uid, -1)
    }

    /// `PowerCmd.Remove`, then the power's `AfterRemoved`.
    fn remove_power(&mut self, owner: u8, uid: u16) {
        let c = self.f.creature_mut(owner);
        let Some(i) = c.by_uid(uid) else {
            return;
        };
        let kind = c.powers[i].kind;
        let n = usize::from(c.n_powers);
        c.powers.copy_within(i + 1..n, i);
        c.n_powers -= 1;
        c.remask();
        self.after_removed(kind);
    }

    /// `PowerModel.AfterApplied` of a new instance.
    fn after_applied(&mut self, owner: u8, p: P) -> Flow {
        match p {
            P::Ringing | P::Tangled => {
                let (cards, n) = self.all_pile_cards();
                let k = self.k;
                for &card in &cards[..n] {
                    let spec = &k.cards[usize::from(self.f.card(card).spec)];
                    if p == P::Ringing {
                        if self.f.card(card).affliction == Affliction::None {
                            self.afflict(card, Affliction::Ringing);
                        }
                    } else if spec.card_type == CardType::Attack {
                        self.afflict(card, Affliction::Entangled);
                    }
                }
                Ok(())
            }
            P::Illusion if !self.f.creature(owner).has(P::Minion) => {
                self.apply_power(owner, P::Minion, 1, NONE)
            }
            P::Ritual if self.f.creature(owner).enemy => {
                let c = self.f.creature_mut(owner);
                if let Some(i) = c.find(P::Ritual) {
                    c.powers[i].data = 1;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// `PowerModel.AfterRemoved`: Ringing and Tangled clear their afflictions.
    fn after_removed(&mut self, p: P) {
        let clear = match p {
            P::Ringing => Affliction::Ringing,
            P::Tangled => Affliction::Entangled,
            _ => return,
        };
        let (cards, n) = self.all_pile_cards();
        for &card in &cards[..n] {
            let c = &mut self.f.cards[usize::from(card)];
            if c.affliction == clear {
                c.affliction = Affliction::None;
            }
        }
    }

    // ----- hook passes -----------------------------------------------------------------------

    /// `Hook.AfterSideTurnEnd` (guarded).
    fn after_side_turn_end(&mut self, enemy_side: bool, participants: &[u8]) -> Flow {
        if self.f.over_or_ending() {
            return Ok(());
        }
        if !self.f.any_listener(SIDE_TURN_END) {
            return Ok(());
        }
        let snap = self.f.snapshot(SIDE_TURN_END)?;
        for entry in &snap.entries[..snap.len] {
            if !self.f.listens(entry.owner) {
                continue;
            }
            let me = self.f.instance(entry);
            let owner = entry.owner;
            let participant = participants.contains(&owner);
            match me.kind {
                P::Vulnerable | P::Weak | P::Frail if enemy_side => {
                    // TickDownDuration.
                    let c = self.f.creature_mut(owner);
                    match c.by_uid(me.uid) {
                        Some(i) if c.powers[i].skip_tick => c.powers[i].skip_tick = false,
                        Some(_) => self.decrement(owner, me.uid)?,
                        None => return Err(Stop),
                    }
                }
                P::Territorial if participant => {
                    self.apply_power(owner, P::Strength, me.amount, owner)?;
                }
                P::Ringing | P::Tangled if participant => self.remove_power(owner, me.uid),
                P::Shrink if participant && me.amount >= 0 => self.decrement(owner, me.uid)?,
                P::Constrict if participant => {
                    self.damage(owner, me.amount, Props::UNPOWERED, owner)?;
                }
                P::Skittish if enemy_side != self.f.creature(owner).enemy => {
                    self.set_data(owner, me.uid, 0);
                }
                P::Asleep if participant => self.decrement(owner, me.uid)?,
                P::Intangible if enemy_side => self.decrement(owner, me.uid)?,
                P::Ritual if participant => {
                    if me.data != 0 {
                        let c = self.f.creature_mut(owner);
                        if let Some(i) = c.by_uid(me.uid) {
                            c.powers[i].data = 0;
                        }
                    } else {
                        self.apply_power(owner, P::Strength, me.amount, owner)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `Hook.AfterSideTurnStart` (guarded): `SlowPower` resets its count; `PlatingPower` loses a
    /// stack after its owner's first turn.
    fn after_side_turn_start(&mut self, participants: &[u8]) -> Flow {
        if self.f.over_or_ending() || !self.f.any_listener(SIDE_TURN_START) {
            return Ok(());
        }
        let snap = self.f.snapshot(SIDE_TURN_START)?;
        for entry in &snap.entries[..snap.len] {
            let owner = entry.owner;
            if !self.f.listens(owner) || !participants.contains(&owner) {
                continue;
            }
            let me = self.f.instance(entry);
            match me.kind {
                P::Slow => {
                    let c = self.f.creature_mut(owner);
                    if let Some(i) = c.by_uid(me.uid) {
                        c.powers[i].data = 0;
                    }
                }
                P::Plating => {
                    let first = if owner == PLAYER {
                        self.f.turn == 1
                    } else {
                        self.f.round == 1
                    };
                    if !first {
                        self.decrement(owner, me.uid)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `Hook.BeforeSideTurnStart` (guarded): `HardenedShellPower` resets its count; an enemy's
    /// `PlatingPower` gives its Block when the first player turn starts (sts2-core plays that
    /// turn start before the kernel takes over).
    fn before_side_turn_start(&mut self, enemy_side: bool) -> Flow {
        if self.f.over_or_ending() {
            return Ok(());
        }
        if !enemy_side && self.f.round <= 1 && self.f.any_listener(PLATING) {
            return Err(Stop);
        }
        if !self.f.any_listener(P::HardenedShell.bit()) {
            return Ok(());
        }
        let snap = self.f.snapshot(P::HardenedShell.bit())?;
        for entry in &snap.entries[..snap.len] {
            if self.f.listens(entry.owner) {
                self.set_data(entry.owner, entry.power.uid, 0);
            }
        }
        Ok(())
    }

    /// Sets the `data` of the instance `uid` of `owner`, if still attached.
    fn set_data(&mut self, owner: u8, uid: u16, data: i32) {
        let c = self.f.creature_mut(owner);
        if let Some(i) = c.by_uid(uid) {
            c.powers[i].data = data;
        }
    }

    /// `Hook.BeforeSideTurnEnd` (guarded), its `VeryEarly` pass (`AsleepPower` with one turn
    /// left drops its owner's Plating) and its `Early` pass (`PlatingPower` gives its owner
    /// `Amount` unpowered Block).
    fn before_side_turn_end(&mut self, participants: &[u8]) -> Flow {
        if self.f.over_or_ending() || !self.f.any_listener(PLATING | P::Asleep.bit()) {
            return Ok(());
        }
        let snap = self.f.snapshot(P::Asleep.bit())?;
        for entry in &snap.entries[..snap.len] {
            let owner = entry.owner;
            if self.f.listens(owner) && participants.contains(&owner) {
                let me = self.f.instance(entry);
                if me.amount <= 1 {
                    self.remove_first(owner, P::Plating);
                }
            }
        }
        if self.f.over_or_ending() {
            return Ok(());
        }
        let snap = self.f.snapshot(PLATING)?;
        for entry in &snap.entries[..snap.len] {
            let owner = entry.owner;
            if self.f.listens(owner) && participants.contains(&owner) {
                let me = self.f.instance(entry);
                self.gain_block(owner, me.amount, Props::UNPOWERED);
            }
        }
        Ok(())
    }

    /// `PowerCmd.Remove(owner.GetPower<T>())` if it has one.
    fn remove_first(&mut self, owner: u8, p: P) {
        let c = self.f.creature(owner);
        if let Some(i) = c.find(p) {
            let uid = c.powers[i].uid;
            self.remove_power(owner, uid);
        }
    }

    /// `Hook.BeforeAttack` (guarded): the attacker's `VigorPower` remembers its first attack.
    /// Returns the instance this attack is remembered by.
    fn before_attack(&mut self, attacker: u8) -> Option<u16> {
        if self.f.creature(attacker).mask & P::Vigor.bit() == 0 || self.f.over_or_ending() {
            return None;
        }
        let c = self.f.creature_mut(attacker);
        let i = c.find(P::Vigor)?;
        let power = &mut c.powers[i];
        if power.data != 0 {
            return None;
        }
        // `commandToModify` and `amountWhenAttackStarted`: kept as the amount (a Vigor amount is
        // at least 1, so 0 means no command).
        power.data = power.amount;
        Some(power.uid)
    }

    /// `Hook.BeforeDamageReceived`: the target's `ThornsPower` hits the dealer of a powered attack.
    fn before_damage_received(&mut self, target: u8, dealer: u8, props: Props) -> Flow {
        if self.f.creature(target).mask & THORNS == 0 || !props.powered() || dealer == NONE {
            return Ok(());
        }
        let snap = self.f.snapshot(THORNS)?;
        for entry in &snap.entries[..snap.len] {
            if entry.owner != target || !self.f.listens(target) {
                continue;
            }
            let me = self.f.instance(entry);
            self.damage(dealer, me.amount, Props::UNPOWERED, target)?;
        }
        Ok(())
    }

    /// `Hook.AfterAttack` (guarded): the attacker's `SuckPower` gains Strength per hit that took
    /// HP; the `VigorPower` this attack is remembered by loses the amount it had; the target's
    /// `SkittishPower` gains Block once a turn when a card's attack took HP from it.
    fn after_attack(
        &mut self,
        attacker: u8,
        target: u8,
        from_card: bool,
        took_hp: i32,
        first_hit: Option<i32>,
        vigor: Option<u16>,
    ) -> Flow {
        if self.f.over_or_ending() || !self.f.any_listener(AFTER_ATTACK) {
            return Ok(());
        }
        let snap = self.f.snapshot(AFTER_ATTACK)?;
        for entry in &snap.entries[..snap.len] {
            let owner = entry.owner;
            if !self.f.listens(owner) {
                continue;
            }
            let me = self.f.instance(entry);
            match me.kind {
                P::Suck if owner == attacker && took_hp > 0 => {
                    self.apply_power(owner, P::Strength, me.amount * took_hp, owner)?;
                }
                P::Vigor if vigor == Some(me.uid) => {
                    self.modify_amount(owner, me.uid, -me.data)?;
                }
                P::Skittish
                    if me.data == 0
                        && from_card
                        && owner == target
                        && first_hit.unwrap_or(0) != 0 =>
                {
                    self.set_data(owner, me.uid, 1);
                    self.gain_block(owner, me.amount, Props::UNPOWERED);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `Hook.AfterCardPlayed`: every listening `SlowPower` counts the play.
    fn after_card_played(&mut self) {
        let enemies = self.f.enemies;
        let n = usize::from(self.f.n_enemies);
        for &owner in std::iter::once(&PLAYER).chain(&enemies[..n]) {
            if !self.f.listens(owner) {
                continue;
            }
            let c = self.f.creature_mut(owner);
            if c.has(P::Slow) {
                for p in &mut c.powers[..usize::from(c.n_powers)] {
                    if p.kind == P::Slow {
                        p.data += 1;
                    }
                }
            }
        }
    }

    /// `Hook.AfterDamageReceived`: `SlipperyPower` uses a stack; `PlowPower`, `ShriekPower` and
    /// `AsleepPower` stun their owner; `HardenedShellPower` counts the HP lost.
    fn after_damage_received(&mut self, target: u8, hit: Hit) -> Flow {
        if self.f.creature(target).mask & AFTER_DAMAGE_RECEIVED == 0 {
            return Ok(());
        }
        let snap = self.f.snapshot(AFTER_DAMAGE_RECEIVED)?;
        let stun_follow_up = |eng: &Self| {
            let kind = usize::from(eng.f.creature(target).monster.kind);
            eng.k.monsters[kind].stun_follow_up
        };
        let unblocked = hit.unblocked;
        for entry in &snap.entries[..snap.len] {
            if entry.owner != target || !self.f.listens(entry.owner) {
                continue;
            }
            let me = self.f.instance(entry);
            match me.kind {
                P::Slippery if unblocked >= 1 => self.decrement(target, me.uid)?,
                P::Plow => {
                    let c = self.f.creature(target);
                    if unblocked <= 0 || c.hp > c.amount(P::Plow) {
                        continue;
                    }
                    self.remove_first(target, P::Strength);
                    let next = stun_follow_up(self);
                    self.stun(target, next);
                    self.remove_power(target, me.uid);
                }
                P::Shriek => {
                    if unblocked <= 0 || self.f.creature(target).hp > me.amount {
                        continue;
                    }
                    let next = stun_follow_up(self);
                    self.stun(target, next);
                    self.remove_power(target, me.uid);
                }
                P::Asleep if unblocked != 0 => {
                    self.remove_first(target, P::Plating);
                    let next = stun_follow_up(self);
                    self.stun(target, next);
                    self.remove_power(target, me.uid);
                }
                P::HardenedShell if !hit.fully_blocked => {
                    self.set_data(target, me.uid, me.data + unblocked);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `Hook.AfterDeath` (no supported death preventer).
    fn after_death(&mut self, dying: u8) -> Flow {
        if !self.f.any_listener(AFTER_DEATH) {
            return Ok(());
        }
        let snap = self.f.snapshot(AFTER_DEATH)?;
        for entry in &snap.entries[..snap.len] {
            if !self.f.listens(entry.owner) {
                continue;
            }
            let me = self.f.instance(entry);
            let owner = entry.owner;
            match me.kind {
                P::Shrink | P::Constrict if me.applier == dying => self.remove_power(owner, me.uid),
                P::Infested if owner == dying => {
                    for slot in WRIGGLER_SLOTS {
                        let slot = self.slot_index(slot);
                        self.add_enemy(WRIGGLER, slot)?;
                    }
                }
                P::Ravenous => {
                    let c = self.f.creature(owner);
                    if dying == owner || self.f.creature(dying).enemy != c.enemy || !c.alive() {
                        continue;
                    }
                    self.stun(owner, None);
                    self.apply_power(owner, P::Strength, me.amount, owner)?;
                }
                P::Illusion if owner == dying => {
                    let c = self.f.creature_mut(owner);
                    if let Some(i) = c.by_uid(me.uid) {
                        c.powers[i].data = 1;
                    }
                    let m = &mut c.monster;
                    if m.log_len == 0 {
                        return Err(Stop);
                    }
                    let follow_up = m.recent[0];
                    set_move_immediate(m, Forced::Revive, follow_up);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// `Hook.AfterCardEnteredCombat` (guarded): Ringing and Tangled afflict the new card.
    fn after_card_entered_combat(&mut self, card: u8) {
        if self.f.over_or_ending() {
            return;
        }
        let player = self.f.player();
        if player.mask & AFTER_CARD_ENTERED == 0 || !self.f.hooks_active {
            return;
        }
        let Ok(snap) = self.f.snapshot(AFTER_CARD_ENTERED) else {
            return;
        };
        for entry in &snap.entries[..snap.len] {
            if entry.owner != PLAYER || !self.f.listens(PLAYER) {
                continue;
            }
            let attack =
                self.k.cards[usize::from(self.f.card(card).spec)].card_type == CardType::Attack;
            if self.f.card(card).affliction != Affliction::None {
                continue;
            }
            match entry.power.kind {
                P::Ringing => self.afflict(card, Affliction::Ringing),
                P::Tangled if attack => self.afflict(card, Affliction::Entangled),
                _ => {}
            }
        }
    }

    // ----- monsters --------------------------------------------------------------------------

    /// `MonsterModel.RollMove` on `Rng.MonsterAi`.
    fn roll_move(&mut self, id: u8) -> Flow {
        let k = self.k;
        let mut m = self.f.creature(id).monster;
        let states = &k.monsters[usize::from(m.kind)].states;
        let current_is_move = m.forced != Forced::None || states[usize::from(m.current)].is_move();
        if !can_transition_away(&m) || (!m.performed_first_move && current_is_move) {
            return Ok(());
        }
        loop {
            m.current = if m.forced == Forced::None {
                self.next_state(id, &m)?
            } else {
                m.forced_follow_up
            };
            m.forced = Forced::None;
            m.forced_follow_up = 0;
            m.forced_performed = false;
            if states[usize::from(m.current)].is_move() {
                break;
            }
        }
        let current = m.current;
        log(&mut m, current);
        self.f.creature_mut(id).monster = m;
        Ok(())
    }

    /// `GetNextState` of the current table state.
    fn next_state(&mut self, id: u8, m: &Monster) -> Flow<u8> {
        let k = self.k;
        let spec = &k.monsters[usize::from(m.kind)];
        match &spec.states[usize::from(m.current)] {
            StateSpec::Move { follow_up, .. } => Ok(*follow_up),
            StateSpec::Conditional { branches } => {
                let c = self.f.creature(id);
                let slots = k.encounters[usize::from(self.f.encounter)].slots;
                branches
                    .iter()
                    .find(|(_, cond)| match *cond {
                        Cond::Front => c.monster.aux != 0,
                        Cond::NotFront => c.monster.aux == 0,
                        Cond::Slot(name) => {
                            c.slot != NONE && slots.get(usize::from(c.slot)) == Some(&name)
                        }
                        Cond::Has(p) => c.has(p),
                        Cond::Lacks(p) => !c.has(p),
                    })
                    .map(|&(s, _)| s)
                    .ok_or(Stop)
            }
            StateSpec::Random { branches } => {
                let mut weights = [0.0_f32; 8];
                if branches.len() > weights.len() {
                    return Err(Stop);
                }
                for (w, b) in weights.iter_mut().zip(branches) {
                    if allows(m, b.state, b.repeat, b.cooldown) {
                        *w = b.weight;
                    }
                }
                let n = branches.len();
                let pick = with_rng(&mut self.f.rng_ai, |rng| rng.random_branch(&weights[..n]));
                pick.map(|i| branches[i].state).ok_or(Stop)
            }
        }
    }

    /// `MonsterModel.PerformMove`.
    fn perform_move(&mut self, id: u8) -> Flow {
        let k = self.k;
        let m = self.f.creature(id).monster;
        self.f.creature_mut(id).monster.performing = true;
        match m.forced {
            Forced::None => {
                let spec = &k.monsters[usize::from(m.kind)];
                let StateSpec::Move { ops, .. } = &spec.states[usize::from(m.current)] else {
                    return Err(Stop);
                };
                self.run_move_ops(id, ops)?;
            }
            Forced::Stunned => self.f.creature_mut(id).monster.forced_performed = true,
            Forced::Revive => {
                // IllusionPower.ReviveMove: no longer reviving, heal to full.
                self.f.creature_mut(id).monster.forced_performed = true;
                let c = self.f.creature_mut(id);
                if let Some(i) = c.find(P::Illusion) {
                    c.powers[i].data = 0;
                }
                let missing = c.max_hp - c.hp;
                self.heal(id, missing);
            }
        }
        let c = self.f.creature_mut(id);
        c.monster.performed_first_move = true;
        c.monster.performing = false;
        // A creature that died during its own move leaves combat now, unless its own
        // IllusionPower keeps it.
        let c = self.f.creature(id);
        if !c.alive() && (!self.f.contains(id) || !c.has(P::Illusion)) {
            self.remove_enemy(id);
        }
        Ok(())
    }

    /// A monster move's (or `AfterAddedToRoom`'s) ops.
    fn run_move_ops(&mut self, id: u8, ops: &[MoveOp]) -> Flow {
        let asc = self.f.ascension;
        for &op in ops {
            match op {
                MoveOp::Attack { damage, hits } => {
                    self.attack(id, NONE, damage.get(asc), hits.get(asc), false)?;
                }
                MoveOp::Block(amount) => self.gain_block(id, amount.get(asc), Props::MOVE),
                MoveOp::ApplySelf(p, amount) => self.apply_power(id, p, amount.get(asc), id)?,
                MoveOp::ApplyPlayers(p, amount) => {
                    self.apply_power(PLAYER, p, amount.get(asc), id)?;
                }
                MoveOp::AddCards {
                    card,
                    count,
                    direct,
                    random_draw,
                } => self.add_status_cards(card, count, direct, random_draw)?,
                MoveOp::Summon { monster, slot } => {
                    let slot = self.slot_index(slot);
                    self.add_enemy(monster, slot)?;
                }
            }
        }
        Ok(())
    }

    /// `Creature.StunInternal`: `STUNNED`, then `next` (or the last logged move).
    fn stun(&mut self, id: u8, next: Option<u8>) {
        let c = self.f.creature_mut(id);
        if !c.alive() {
            return;
        }
        let m = &mut c.monster;
        let follow_up = next.unwrap_or(m.recent[0]);
        set_move_immediate(m, Forced::Stunned, follow_up);
    }

    /// The index of the encounter slot `name`, or [`NONE`].
    fn slot_index(&self, name: &str) -> u8 {
        self.k.encounters[usize::from(self.f.encounter)]
            .slots
            .iter()
            .position(|&s| s == name)
            .and_then(|i| u8::try_from(i).ok())
            .unwrap_or(NONE)
    }

    /// `CreatureCmd.Add<T>(slot)`: create the creature (HP on `Rng.Niche`), add it, sort the
    /// enemies by slot, `AfterAddedToRoom`, and its first roll during the player's turn.
    fn add_enemy(&mut self, kind: u8, slot: u8) -> Flow {
        let k = self.k;
        let spec = &k.monsters[usize::from(kind)];
        let (min, max) = spec.hp[usize::from(self.f.ascension.min(20))];
        // Creature.SetUniqueMonsterHpValue.
        let mut candidates = [0; 32];
        let mut n = 0;
        for hp in min..=max {
            if self
                .f
                .enemies()
                .iter()
                .any(|&e| self.f.creature(e).max_hp == hp)
            {
                continue;
            }
            if n == candidates.len() {
                return Err(Stop);
            }
            candidates[n] = hp;
            n += 1;
        }
        let hp = with_rng(&mut self.f.rng_niche, |rng| {
            if n == 0 {
                rng.next_int_range(min, max + 1)
            } else {
                *rng.next_item(&candidates[..n]).expect("candidates")
            }
        });
        let id = self.f.n_creatures;
        if usize::from(id) >= MAX_CREATURES || usize::from(self.f.n_enemies) >= MAX_CREATURES - 1 {
            return Err(Stop);
        }
        let mut monster = Monster {
            kind,
            current: spec.initial,
            spawned_this_turn: true,
            ..Monster::default()
        };
        if spec.states[usize::from(spec.initial)].is_move() {
            log(&mut monster, spec.initial);
        }
        self.f.creatures[usize::from(id)] = crate::state::Creature {
            hp,
            max_hp: hp,
            enemy: true,
            slot,
            monster,
            ..crate::state::Creature::default()
        };
        self.f.n_creatures += 1;
        self.f.enemies[usize::from(self.f.n_enemies)] = id;
        self.f.n_enemies += 1;
        if slot != NONE {
            self.sort_enemies_by_slot();
        }
        self.run_move_ops(id, &spec.after_added)?;
        if !self.f.enemy_side {
            self.roll_move(id)?;
        }
        Ok(())
    }

    /// `SortEnemiesBySlotName`: .NET's sort for up to 16 items.
    fn sort_enemies_by_slot(&mut self) {
        let fight = &*self.f;
        let count = usize::from(fight.n_enemies);
        let key = |id: u8| -> i16 {
            let slot = fight.creature(id).slot;
            if slot == NONE { -1 } else { i16::from(slot) }
        };
        let greater = |a: u8, b: u8| key(a) > key(b);
        let mut list = fight.enemies;
        match count {
            0 | 1 => {}
            2 => {
                if greater(list[0], list[1]) {
                    list.swap(0, 1);
                }
            }
            3 => {
                for (i, j) in [(0, 1), (0, 2), (1, 2)] {
                    if greater(list[i], list[j]) {
                        list.swap(i, j);
                    }
                }
            }
            _ => {
                for i in 0..count - 1 {
                    let item = list[i + 1];
                    let mut j = i + 1;
                    while j > 0 && greater(list[j - 1], item) {
                        list[j] = list[j - 1];
                        j -= 1;
                    }
                    list[j] = item;
                }
            }
        }
        self.f.enemies = list;
    }
}

/// A stable sort of up to 16 `(rank, card)` pairs by rank.
fn stable_sort_by_rank(list: &mut [(u16, u8)]) {
    let mut sorted = [(0, 0); 16];
    let mut n = 0;
    let mut rank = 0;
    while n < list.len() {
        let mut next = u16::MAX;
        for &item in list.iter() {
            if item.0 == rank {
                sorted[n] = item;
                n += 1;
            } else if item.0 > rank && item.0 < next {
                next = item.0;
            }
        }
        rank = next;
    }
    list.copy_from_slice(&sorted[..list.len()]);
}

/// `CanTransitionAway`: every supported forced move must be performed once first.
const fn can_transition_away(m: &Monster) -> bool {
    matches!(m.forced, Forced::None) || m.forced_performed
}

/// `MonsterModel.SetMoveImmediate`.
const fn set_move_immediate(m: &mut Monster, kind: Forced, follow_up: u8) {
    if can_transition_away(m) {
        m.forced = kind;
        m.forced_follow_up = follow_up;
        m.forced_performed = false;
    }
}

/// Appends `state` to the machine's log.
const fn log(m: &mut Monster, state: u8) {
    m.recent = [state, m.recent[0], m.recent[1]];
    m.log_len = m.log_len.saturating_add(1);
    m.used |= 1 << state;
}

/// `RandomBranchState.GetStateWeight`'s repeat and cooldown rules.
fn allows(m: &Monster, state: u8, repeat: Repeat, cooldown: u8) -> bool {
    // `tables.rs` keeps every repeat run and cooldown within the three logged states.
    let logged = usize::from(m.log_len.min(3));
    let run = |n: usize| logged >= n && m.recent[..n].iter().all(|&s| s == state);
    let allowed = match repeat {
        Repeat::CanRepeatForever => true,
        Repeat::UseOnlyOnce => m.used & (1 << state) == 0,
        Repeat::CannotRepeat => !run(1),
        Repeat::CanRepeatXTimes(max) => !run(usize::try_from(max).unwrap_or(3).min(3)),
    };
    let cooling = cooldown > 0 && m.recent[..logged.min(usize::from(cooldown))].contains(&state);
    allowed && !cooling
}
