//! Read-only questions a front end asks about a fight: why a card cannot be played
//! ([`CombatState::unplayable_reason`]), what playing it would certainly do
//! ([`Combat::play_outcome`]) and what the enemies' next turn does to the player
//! ([`Combat::incoming`]). Nothing here changes the fight: each runs the engine on a copy.

use super::card_play::AutoPlayType;
use super::engine::{Engine, Event, Interrupt};
use super::hooks::{Hooked, Listener};
use super::state::{CardId, CombatResult, CombatState, CreatureId, Creatures, TurnPhase};
use super::{Combat, Intent, IntentType, Status, Step, execute_on};
use crate::content::cards::{CardKeyword, CardKind};
use crate::content::potions::PotionKind;
use crate::content::powers::PowerKind;
use crate::content::relics::RelicKind;
use crate::rng::{Rng, RunRngSet, xxh64};

/// Why a card cannot be played, as the game's thought bubble tells it
/// (`UnplayableReasonExtensions.GetPlayerDialogueLine`): the first of these that holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnplayableReason {
    /// `EnergyCostTooHigh`: `combat_messages.NOT_ENOUGH_ENERGY`.
    NotEnoughEnergy,
    /// `StarCostTooHigh`: `combat_messages.NOT_ENOUGH_STARS`.
    NotEnoughStars,
    /// `BlockedByHook` with its preventer: `combat_messages.BLOCKED_BY_HOOK`.
    BlockedBy(Preventer),
    /// `HasUnplayableKeyword` or `BlockedByCardLogic` (`IsPlayable`):
    /// `combat_messages.UNPLAYABLE`.
    Unplayable,
}

/// The model whose `ShouldPlay` blocks a card, for its title.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Preventer {
    /// A power of this kind.
    Power(PowerKind),
    /// This monster.
    Monster(CreatureId),
    /// A relic of this kind.
    Relic(RelicKind),
    /// A potion of this kind.
    Potion(PotionKind),
    /// A card (`Normality`).
    Card(CardId),
    /// The affliction on this card.
    Affliction(CardId),
    /// The enchantment on this card.
    Enchantment(CardId),
}

impl Hooked<'_> {
    /// [`CombatState::unplayable_reason`] on these listeners.
    #[must_use]
    pub fn unplayable_reason(self, card: CardId) -> Option<UnplayableReason> {
        let s = self.s;
        if super::can_play_with(self, card).is_ok() {
            return None;
        }
        // `HasEnoughResourcesFor` (`PlayerCombatState.cs:190-209`): an X cost needs 0.
        if s.player.energy < self.energy_cost(card).max(0) {
            return Some(UnplayableReason::NotEnoughEnergy);
        }
        if s.player.stars < self.star_cost(card).max(0) {
            return Some(UnplayableReason::NotEnoughStars);
        }
        if let Some(listener) = self.play_preventer(card, AutoPlayType::None) {
            return Some(UnplayableReason::BlockedBy(s.preventer(listener)));
        }
        debug_assert!(
            self.has_keyword(card, CardKeyword::Unplayable)
                || !crate::content::cards::is_playable(s, card),
            "`can_play` failed for a reason this does not know"
        );
        Some(UnplayableReason::Unplayable)
    }
}

impl CombatState {
    /// `CardModel.CanPlay`'s reason (`CardModel.cs:1725-1754`) as the dialogue line picks it;
    /// `None` when the card can be played. `NoLivingAllies` is multiplayer only.
    #[must_use]
    pub fn unplayable_reason(&self, card: CardId) -> Option<UnplayableReason> {
        self.hooked().unplayable_reason(card)
    }

    /// `NHandCardHolder.ShouldGlowGold` outside a hand selection: the card can be played, the
    /// player is in the Play phase, and its `ShouldGlowGold` holds (`ShouldGlowGoldInternal`; no
    /// enchantment overrides `EnchantmentModel.ShouldGlowGold`). Display only.
    #[must_use]
    pub fn glows_gold(&self, card: CardId) -> bool {
        self.player.phase == TurnPhase::Play
            && super::can_play(self, card).is_ok()
            && crate::content::cards::glows_gold(self, card)
    }

    fn preventer(&self, listener: Listener) -> Preventer {
        match listener {
            Listener::Power { owner, power } => Preventer::Power(
                self.creature(owner)
                    .powers
                    .iter()
                    .find(|p| p.id == power)
                    .expect("the preventer power exists")
                    .kind,
            ),
            Listener::Monster(id) => Preventer::Monster(id),
            Listener::Relic(id) => Preventer::Relic(
                self.player
                    .relics
                    .iter()
                    .find(|r| r.id == id)
                    .expect("the preventer relic exists")
                    .kind,
            ),
            Listener::Potion(id) => Preventer::Potion(
                self.player
                    .potions()
                    .find(|p| p.id == id)
                    .expect("the preventer potion exists")
                    .kind,
            ),
            Listener::Card(id) => Preventer::Card(id),
            Listener::Affliction(id) => Preventer::Affliction(id),
            Listener::Enchantment(id) => Preventer::Enchantment(id),
        }
    }
}

/// What a play does to one creature ([`Combat::play_outcome`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CreatureOutcome {
    /// The creature.
    pub creature: CreatureId,
    /// HP before the play.
    pub hp_before: i32,
    /// HP after it.
    pub hp_after: i32,
    /// Block before the play.
    pub block_before: i32,
    /// Block after it.
    pub block_after: i32,
    /// Alive before the play and dead after it.
    pub dies: bool,
}

/// What a play certainly does ([`Combat::play_outcome`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlayOutcome {
    /// Every creature whose HP, Block or life the play changes, in creature order.
    pub creatures: Vec<CreatureOutcome>,
    /// The play ends the fight in a win.
    pub wins: bool,
    /// The play goes on to draw or shuffle ([`Combat::play_outcome_until_draw`] only):
    /// `creatures` is what it does before that, and what follows is left out.
    pub partial: bool,
}

impl PlayOutcome {
    /// The outcome for `creature`, if the play changes it.
    #[must_use]
    pub fn of(&self, creature: CreatureId) -> Option<&CreatureOutcome> {
        self.creatures.iter().find(|c| c.creature == creature)
    }
}

/// Reseeded random streams a preview replays with to tell whether its result depends on random
/// numbers.
pub(super) const RESEEDS: [&str; 4] = ["PREVIEWA", "PREVIEWB", "PREVIEWC", "PREVIEWD"];

/// `state` with every random stream reseeded from `seed` ([`reseed`]).
pub(super) fn reseeded(state: &CombatState, seed: &str) -> CombatState {
    let mut state = state.clone();
    reseed(&mut state, seed);
    state
}

/// Reseeds every random stream of `state` from `seed`: the run's and each monster's own.
pub(super) fn reseed(state: &mut CombatState, seed: &str) {
    state.rng = RunRngSet::new(seed);
    for (i, c) in state.creatures.iter_mut().enumerate() {
        if let Some(m) = c.monster.as_mut() {
            m.rng = Rng::new(
                xxh64(seed).wrapping_add(u64::try_from(i).expect("a creature index fits")),
            );
        }
    }
}

/// Whether no random number was drawn between `before` and `after` (a later state of the same
/// fight): the run's streams and every monster's own stream are where they were.
pub(super) fn same_rng(before: &CombatState, after: &CombatState) -> bool {
    before.rng == after.rng
        && before.creatures.iter().zip(&after.creatures).all(|(b, a)| {
            match (&b.monster, &a.monster) {
                (Some(b), Some(a)) => b.rng == a.rng,
                _ => true,
            }
        })
}

impl Combat {
    /// What playing `card` at `target` would do, worked out on a copy of the fight, only when
    /// the result follows from what the player can see: `None` if the play is not legal now,
    /// stops for a choice, draws or shuffles (the draw pile's order is hidden), or uses a
    /// random number (whose result the game hides; the monsters' own streams included).
    ///
    /// A play that wins the fight sets off the end of the fight, whose rewards draw random
    /// numbers after the last kill; such a play counts as certain when it ends the same under
    /// every one of [`RESEEDS`] (each stream reseeded) as under the real streams.
    #[must_use]
    pub fn play_outcome(&self, card: CardId, target: Option<CreatureId>) -> Option<PlayOutcome> {
        self.outcome_of(card, target)
            .filter(|outcome| !outcome.partial)
    }

    /// [`Combat::play_outcome`], except that a play that draws or shuffles gives what it
    /// certainly does before that, marked [`PlayOutcome::partial`] (Pommel Strike's damage).
    #[must_use]
    pub fn play_outcome_until_draw(
        &self,
        card: CardId,
        target: Option<CreatureId>,
    ) -> Option<PlayOutcome> {
        self.outcome_of(card, target)
    }

    /// [`Combat::play_outcome_until_draw`].
    fn outcome_of(&self, card: CardId, target: Option<CreatureId>) -> Option<PlayOutcome> {
        if self.status() != Status::PlayerTurn || self.check_play(card, target).is_err() {
            return None;
        }
        let (after, partial) = Self::scratch_play(self.state.clone(), card, target)?;
        let outcome = outcome(&self.state, &after, partial);
        if outcome.wins {
            let same = RESEEDS.iter().all(|seed| {
                Self::scratch_play(reseeded(&self.state, seed), card, target)
                    .is_some_and(|(a, p)| outcome == self::outcome(&self.state, &a, p))
            });
            return same.then_some(outcome);
        }
        same_rng(&self.state, &after).then_some(outcome)
    }

    /// Plays `card` (a legal play) on a copy of this fight with `state`, up to its first draw
    /// or shuffle: the state after it and whether it stopped there; `None` if it stops for a
    /// choice or at unported content.
    fn scratch_play(
        mut state: CombatState,
        card: CardId,
        target: Option<CreatureId>,
    ) -> Option<(CombatState, bool)> {
        let mut eng = Engine::new(&mut state, None, &[]);
        eng.stop_at_hidden = true;
        let partial = match execute_on(&mut eng, &Step::PlayCard { card, target }) {
            Ok(()) => false,
            Err(interrupt) if *interrupt == Interrupt::Hidden => true,
            Err(_) => return None,
        };
        Some((state, partial))
    }
}

/// The creatures a play changed, from the state `before` it to the state `after` it.
fn outcome(before: &CombatState, after: &CombatState, partial: bool) -> PlayOutcome {
    let creatures = before
        .creatures
        .iter()
        .zip(&after.creatures)
        .enumerate()
        .filter_map(|(i, (b, a))| {
            let id = CreatureId(u32::try_from(i).ok()?);
            let dies = b.is_alive() && !a.is_alive();
            (b.hp != a.hp || b.block != a.block || dies).then_some(CreatureOutcome {
                creature: id,
                hp_before: b.hp,
                hp_after: a.hp,
                block_before: b.block,
                block_after: a.block,
                dies,
            })
        })
        .collect();
    PlayOutcome {
        creatures,
        wins: after.result == Some(CombatResult::Won),
        partial,
    }
}

/// What the enemies' next turn does to the player ([`Combat::incoming`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IncomingPreview {
    /// Per living enemy in slot order (the order they act).
    pub enemies: Vec<EnemyIncoming>,
    /// Every hit the intents show, summed, before Block.
    pub total: i32,
    /// The player's Block spent on damage until the enemy turn ends, hit by hit.
    pub blocked: i32,
    /// HP each of the player's pets (Osty) loses until the enemy turn ends, absorbing damage
    /// instead of the player: one entry per pet hit, in the order first hit.
    pub absorbed_by: Vec<(CreatureId, i32)>,
    /// HP the player loses to damage from the end of this turn to the end of the enemy turn,
    /// after Block, Intangible, Buffer, Tungsten Rod, Plated Armor's Block, Osty and every other
    /// hook the engine runs, in hit order.
    pub hp_loss: i32,
    /// The player's HP when the enemy turn ends (after a revive, if one fired).
    pub hp_after: i32,
    /// The player dies and no revive saves them.
    pub lethal: bool,
    /// False when the result may differ from what the player will see: some living enemy's
    /// intent is hidden (its move is left out), the turn draws or shuffles, stops for a
    /// choice, or ends differently under other random numbers. The figures are then a
    /// lower bound at best ("at least").
    pub certain: bool,
}

/// One enemy's part of an [`IncomingPreview`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EnemyIncoming {
    /// The enemy.
    pub creature: CreatureId,
    /// [`CombatState::intents`].
    pub intents: Vec<Intent>,
    /// Every hit its intents show, in order, per-hit damage after modifiers: `[6, 6, 6]`.
    pub hits: Vec<i32>,
    /// `hits` summed.
    pub total: i32,
    /// HP the player loses to damage this enemy deals, given the Block spent before it.
    pub hp_loss: i32,
    /// The powers its intents apply to the player's side ([`Intent::applies`] of its debuff
    /// intents).
    pub applies: Vec<(PowerKind, i32)>,
    /// The status cards its intents add, when known ([`Intent::card`], [`Intent::cards`]).
    pub status_cards: Vec<(CardKind, u32)>,
}

impl EnemyIncoming {
    fn new(s: &CombatState, creature: CreatureId) -> Self {
        let intents = s.intents(creature);
        let hits: Vec<i32> = intents
            .iter()
            .filter_map(|i| i.attack)
            .flat_map(|a| (0..a.hits).map(move |_| a.damage))
            .collect();
        let applies = intents
            .iter()
            .filter(|i| matches!(i.kind, IntentType::Debuff | IntentType::DebuffStrong))
            .flat_map(|i| i.applies.iter().copied())
            .collect();
        let status_cards = intents
            .iter()
            .filter_map(|i| Some((i.card?, i.cards?)))
            .collect();
        Self {
            creature,
            total: hits.iter().sum(),
            hits,
            hp_loss: 0,
            intents,
            applies,
            status_cards,
        }
    }
}

/// What an enemy turn run on a copy did to the player's side ([`Combat::scratch_enemy_turn`]).
#[derive(Debug, PartialEq, Eq)]
struct EnemyTurn {
    /// HP the player lost to each dealer, per dealer in the order first seen.
    by_dealer: Vec<(CreatureId, i32)>,
    blocked: i32,
    absorbed_by: Vec<(CreatureId, i32)>,
    hp_loss: i32,
    hp_after: i32,
    lethal: bool,
    /// It ran to the end of the enemy turn without a draw, a shuffle or a choice.
    finished: bool,
    /// It drew no random number.
    no_rng: bool,
}

/// Adds `amount` to `key`'s entry in `list`.
fn add_to(list: &mut Vec<(CreatureId, i32)>, key: CreatureId, amount: i32) {
    match list.iter_mut().find(|(k, _)| *k == key) {
        Some((_, sum)) => *sum += amount,
        None => list.push((key, amount)),
    }
}

impl Combat {
    /// What the enemies' next turn would do to the player, worked out on a copy of the fight:
    /// the player's turn ends now (end-of-turn effects, Plated Armor, discards) and each enemy
    /// performs its move through the engine's own damage path, up to the end of the enemy
    /// turn. Moves behind a hidden intent are left out. `None` outside the play phase.
    #[must_use]
    pub fn incoming(&self) -> Option<IncomingPreview> {
        if self.status() != Status::PlayerTurn {
            return None;
        }
        let s = &self.state;
        let mut enemies: Vec<EnemyIncoming> = s
            .enemies
            .iter()
            .filter(|&&id| s.creature(id).is_alive())
            .map(|&id| EnemyIncoming::new(s, id))
            .collect();
        let idle: Creatures = enemies
            .iter()
            .filter(|e| {
                e.intents
                    .iter()
                    .any(|i| matches!(i.kind, IntentType::Hidden | IntentType::Unknown))
            })
            .map(|e| e.creature)
            .collect();
        let turn = self.scratch_enemy_turn(s.clone(), &idle);
        let certain = idle.is_empty()
            && turn.finished
            && (turn.no_rng
                || RESEEDS
                    .iter()
                    .all(|seed| self.scratch_enemy_turn(reseeded(s, seed), &idle) == turn));
        for enemy in &mut enemies {
            enemy.hp_loss = turn
                .by_dealer
                .iter()
                .find(|(d, _)| *d == enemy.creature)
                .map_or(0, |&(_, loss)| loss);
        }
        Some(IncomingPreview {
            total: enemies.iter().map(|e| e.total).sum(),
            enemies,
            blocked: turn.blocked,
            absorbed_by: turn.absorbed_by,
            hp_loss: turn.hp_loss,
            hp_after: turn.hp_after,
            lethal: turn.lethal,
            certain,
        })
    }

    /// Ends the player's turn on a copy of this fight with `state` and runs the enemy turn,
    /// the monsters in `idle` not moving, stopping early at a draw, a shuffle or a choice.
    fn scratch_enemy_turn(&self, mut state: CombatState, idle: &[CreatureId]) -> EnemyTurn {
        let mut events = Vec::new();
        let mut eng = Engine::new(&mut state, Some(&mut events), &[]);
        eng.stop_at_hidden = true;
        let finished = eng.end_player_turn_to_enemy_turn_end(idle).is_ok();
        let player = state.player.creature;
        let mut turn = EnemyTurn {
            by_dealer: Vec::new(),
            blocked: 0,
            absorbed_by: Vec::new(),
            hp_loss: 0,
            hp_after: state.creature(player).hp,
            lethal: state.result == Some(CombatResult::Lost) || state.creature(player).is_dead(),
            finished,
            no_rng: same_rng(&self.state, &state),
        };
        for event in &events {
            let Event::Damage {
                dealer,
                receiver,
                blocked,
                hp_lost,
            } = *event
            else {
                continue;
            };
            if receiver == player {
                turn.blocked += blocked;
                turn.hp_loss += hp_lost;
                if let Some(dealer) = dealer {
                    add_to(&mut turn.by_dealer, dealer, hp_lost);
                }
            } else if state.creature(receiver).pet_owner == Some(player) {
                add_to(&mut turn.absorbed_by, receiver, hp_lost);
            }
        }
        turn
    }
}
