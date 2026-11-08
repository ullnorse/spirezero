//! What a monster's intent shows: the intents its next move is built with
//! ([`CombatState::intents`]), each attack's damage (`AttackIntent.GetSingleDamage`,
//! `AttackIntent.cs:107-116`), and the powers and status cards the move gives.

use super::engine::{Engine, Event, ValueProps};
use super::history::HistoryKind;
use super::hooks::DamageQuery;
use super::hooks::Hooked;
use super::preview::{RESEEDS, reseed, same_rng};
use super::state::{CombatState, CreatureId};
use crate::content::cards::CardKind;
use crate::content::monsters;
use crate::content::monsters::machine::ForcedMove;
use crate::content::powers::PowerKind;
use crate::content::tables::{self, Declared};
use crate::decimal::Decimal;

/// `IntentType` (`IntentType.cs`): what an intent shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntentType {
    /// `SingleAttackIntent`, `MultiAttackIntent`.
    Attack,
    /// `IntentType.Buff`.
    Buff,
    /// `IntentType.Debuff`.
    Debuff,
    /// `DebuffIntent(strong: true)`.
    DebuffStrong,
    /// `IntentType.Defend`.
    Defend,
    /// `IntentType.Escape`.
    Escape,
    /// `IntentType.Heal`.
    Heal,
    /// `IntentType.Hidden`.
    Hidden,
    /// `IntentType.Summon`.
    Summon,
    /// `IntentType.Sleep`.
    Sleep,
    /// `IntentType.Stun`.
    Stun,
    /// `StatusIntent`: status cards for the player.
    StatusCard,
    /// `IntentType.CardDebuff`.
    CardDebuff,
    /// `DeathBlowIntent` (`GasBomb`, `WaterfallGiant`): a `SingleAttackIntent` with its own type.
    DeathBlow,
    /// `UnknownIntent` (no monster move is built with one).
    Unknown,
}

/// One intent of a monster's next move.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Intent {
    /// What the intent shows.
    pub kind: IntentType,
    /// An attack intent's damage per hit and hits, if the move makes that attack now.
    pub attack: Option<AttackIntent>,
    /// A `StatusIntent`'s `CardCount`.
    pub cards: Option<u32>,
    /// A `StatusIntent`'s card: the kind of every card the move generates, when they share one
    /// kind whatever the random numbers.
    pub card: Option<CardKind>,
    /// What the move applies, each distinct `(power, amount)` once, when it does so whatever the
    /// random numbers: on a `Buff` intent the powers the monster gives its own side, on a
    /// `Debuff` or `DebuffStrong` intent those it gives the other side (`PowerReceived` entries
    /// with the monster as applier, after the receiver's modifiers). Empty for other kinds.
    pub applies: Vec<(PowerKind, i32)>,
}

/// One attack of a monster's next move, as its intent displays it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttackIntent {
    /// The type of the move's intent that makes this attack.
    pub kind: IntentType,
    /// Damage per hit after the player's and the monster's modifiers.
    pub damage: i32,
    /// The number of hits.
    pub hits: u32,
}

/// A caller-owned scratch for [`Hooked::intents_into`].
///
/// It holds the copy of the state a monster's move runs on, refilled with `clone_from`, and
/// the buffers the run is read into, so recomputing intents reuses their allocations.
#[derive(Debug, Default)]
pub struct IntentScratch {
    state: Option<CombatState>,
    events: Vec<Event>,
    effects: MoveEffects,
    /// The run under other random numbers, for a move that draws one.
    other: MoveEffects,
}

/// Copies `s` into a scratch's `state`; the copy.
fn refill<'a>(state: &'a mut Option<CombatState>, s: &CombatState) -> &'a mut CombatState {
    match state {
        Some(state) => {
            state.clone_from(s);
            state
        }
        None => state.insert(s.clone()),
    }
}

impl CombatState {
    /// `NextMove.Intents` of `monster`, in declaration order, with each attack intent's damage
    /// ([`CombatState::intent_damage`], paired in order) and each `StatusIntent`'s card count.
    /// Empty before the first move is rolled, for a move with no intent, or for a creature
    /// that is not a monster.
    #[must_use]
    pub fn intents(&self, monster: CreatureId) -> Vec<Intent> {
        let mut intents = Vec::new();
        self.hooked().intents_into(
            monster,
            &mut IntentScratch::default(),
            &mut intents,
            &mut Vec::new(),
        );
        intents
    }

    /// `MonsterModel.IntendsToAttack` (`MonsterModel.cs:242`): the next move has an `Attack` or
    /// `DeathBlow` intent.
    #[must_use]
    pub fn intends_to_attack(&self, monster: CreatureId) -> bool {
        self.intent_damage(monster)
            .iter()
            .any(|a| matches!(a.kind, IntentType::Attack | IntentType::DeathBlow))
    }

    /// The attacks of `monster`'s next move, in the order the move makes them; empty for a
    /// move that does not attack.
    ///
    /// The move runs on a scratch copy to find each attack's base damage, so there is no second
    /// damage table. Each base is then modified like the intent does it
    /// ([`Hooked::intent_damage`]).
    #[must_use]
    pub fn intent_damage(&self, monster: CreatureId) -> Vec<AttackIntent> {
        let Some(m) = self.monster_of(monster) else {
            return Vec::new();
        };
        let kind = m.kind.model().attack_intent_type(m.machine.current);
        let h = self.hooked();
        let IntentScratch {
            state,
            events,
            effects,
            ..
        } = &mut IntentScratch::default();
        let state = refill(state, self);
        effects.read_move(self, monster, state, events);
        effects
            .attacks
            .iter()
            .map(|&(base, hits)| AttackIntent {
                kind,
                damage: h.intent_damage(monster, base),
                hits,
            })
            .collect()
    }

    /// `monster`'s monster, if it is a monster in this combat.
    fn monster_of(&self, monster: CreatureId) -> Option<&monsters::Monster> {
        self.contains_creature(monster)
            .then(|| self.creature(monster).monster.as_ref())
            .flatten()
    }
}

impl Hooked<'_> {
    /// `AttackIntent.GetSingleDamage` (`AttackIntent.cs:107-116`) of an attack by `monster`
    /// whose damage per hit before modifiers is `base`: `Hook.ModifyDamage` with the player as
    /// target, the monster as dealer, `ValueProp.Move` and no card, on these listeners; then
    /// `max(0, (int)x)`.
    #[must_use]
    pub fn intent_damage(self, monster: CreatureId, base: Decimal) -> i32 {
        let query = DamageQuery {
            target: Some(self.s.player.creature),
            dealer: Some(monster),
            props: ValueProps::MOVE,
            card: None,
        };
        self.modify_damage(query, base).0.to_int_truncating().max(0)
    }

    /// [`CombatState::intents`] of `monster` into `intents` (replaced; its elements'
    /// allocations are reused), with the move run on `scratch`. `bases` (replaced) gets the
    /// base damage of each intent that has an attack, in order: the value
    /// [`Hooked::intent_damage`] folds into that attack's `damage`, so a caller that keeps the
    /// intents can fold the bases again on a later state of the same move.
    pub fn intents_into(
        self,
        monster: CreatureId,
        scratch: &mut IntentScratch,
        intents: &mut Vec<Intent>,
        bases: &mut Vec<Decimal>,
    ) {
        bases.clear();
        let s = self.s;
        let Some(m) = s.monster_of(monster) else {
            intents.clear();
            return;
        };
        let declared: &[Declared] = match m.machine.forced.map(|f| f.kind) {
            // `Creature.StunInternal` (`Creature.cs:538`).
            Some(ForcedMove::Stunned | ForcedMove::BeetleWakeUp) => &[Declared::Stun],
            // `IllusionPower` (`IllusionPower.cs:83`).
            Some(ForcedMove::IllusionRevive) => &[Declared::Heal],
            _ => tables::declared_intents(m.kind.id(), m.next_move_name()).unwrap_or(&[]),
        };
        let attack_kind = m.kind.model().attack_intent_type(m.machine.current);
        let IntentScratch {
            state,
            events,
            effects,
            other,
        } = scratch;
        let copy = refill(state, s);
        effects.read_move(s, monster, copy, events);
        if effects.random {
            // Keep only what the move gives under other random numbers too.
            let copy = refill(state, s);
            reseed(copy, RESEEDS[0]);
            other.read_move(s, monster, copy, events);
            if other.powers != effects.powers {
                effects.powers.clear();
            }
            if other.cards != effects.cards {
                effects.cards.clear();
            }
        }
        let effects = &*effects;
        let card = effects
            .cards
            .split_first()
            .and_then(|(&first, rest)| rest.iter().all(|&c| c == first).then_some(first));
        let mut attacks = effects.attacks.iter();
        for (i, &d) in declared.iter().enumerate() {
            let kind = intent_type(d);
            let attack = matches!(kind, IntentType::Attack | IntentType::DeathBlow)
                .then(|| attacks.next())
                .flatten()
                .map(|&(base, hits)| {
                    bases.push(base);
                    AttackIntent {
                        kind: attack_kind,
                        damage: self.intent_damage(monster, base),
                        hits,
                    }
                });
            let (cards, card) = match d {
                Declared::StatusCard(count) => (Some(count.at(s.ascension)), card),
                _ => (None, None),
            };
            if let Some(intent) = intents.get_mut(i) {
                intent.kind = kind;
                intent.attack = attack;
                intent.cards = cards;
                intent.card = card;
                intent.applies.clear();
            } else {
                intents.push(Intent {
                    kind,
                    attack,
                    cards,
                    card,
                    applies: Vec::new(),
                });
            }
        }
        intents.truncate(declared.len());
        for &(own_side, power, amount) in &effects.powers {
            let target = intents.iter_mut().find(|i| {
                if own_side {
                    i.kind == IntentType::Buff
                } else {
                    matches!(i.kind, IntentType::Debuff | IntentType::DebuffStrong)
                }
            });
            if let Some(intent) = target
                && !intent.applies.contains(&(power, amount))
            {
                intent.applies.push((power, amount));
            }
        }
    }
}

/// The `IntentType` of a declared intent.
const fn intent_type(d: Declared) -> IntentType {
    match d {
        Declared::SingleAttack | Declared::MultiAttack => IntentType::Attack,
        Declared::DeathBlow => IntentType::DeathBlow,
        Declared::Buff => IntentType::Buff,
        Declared::Debuff => IntentType::Debuff,
        Declared::DebuffStrong => IntentType::DebuffStrong,
        Declared::Defend => IntentType::Defend,
        Declared::Escape => IntentType::Escape,
        Declared::Heal => IntentType::Heal,
        Declared::Hidden => IntentType::Hidden,
        Declared::Summon => IntentType::Summon,
        Declared::Sleep => IntentType::Sleep,
        Declared::Stun => IntentType::Stun,
        Declared::StatusCard(_) => IntentType::StatusCard,
        Declared::CardDebuff => IntentType::CardDebuff,
    }
}

/// What a monster's move did on a scratch copy of the state ([`MoveEffects::read_move`]).
#[derive(Debug, Default)]
struct MoveEffects {
    /// The attacks it made, in order: damage per hit before modifiers, and hits.
    attacks: Vec<(Decimal, u32)>,
    /// Its `PowerReceivedEntry`s with the monster as applier, in order: whether the receiver
    /// is on the monster's side, the power and the amount received (not 0).
    powers: Vec<(bool, PowerKind, i32)>,
    /// The kinds of the cards it generated (`CardGeneratedEntry`), in order.
    cards: Vec<CardKind>,
    /// It gave powers or cards and drew a random number (the run's streams or a monster's
    /// own), so what it gave may differ under other random numbers.
    random: bool,
}

impl MoveEffects {
    /// Runs `monster`'s next move on `scratch` (a copy of `s`, maybe reseeded) and reads what
    /// it did.
    fn read_move(
        &mut self,
        s: &CombatState,
        monster: CreatureId,
        scratch: &mut CombatState,
        events: &mut Vec<Event>,
    ) {
        events.clear();
        let mark = scratch.history_mark();
        let mut eng = Engine::new(scratch, Some(events), &[]);
        // A move that stops for a choice still reports what it did before it.
        let _ = monsters::perform_move(&mut eng, monster);
        self.attacks.clear();
        self.attacks
            .extend(events.iter().filter_map(|event| match event {
                Event::Attack {
                    attacker,
                    damage,
                    hits,
                } if *attacker == monster => Some((*damage, *hits)),
                _ => None,
            }));
        let side = s.creature(monster).side;
        self.powers.clear();
        self.cards.clear();
        for entry in scratch.history_since(mark) {
            match entry.kind {
                HistoryKind::PowerReceived {
                    kind,
                    amount,
                    applier,
                } if applier == Some(monster) => {
                    let amount = amount.to_int_truncating();
                    if amount != 0 {
                        let own_side = scratch.creature(entry.actor).side == side;
                        self.powers.push((own_side, kind, amount));
                    }
                }
                HistoryKind::CardGenerated { card, .. } => {
                    self.cards.push(scratch.card(card).kind);
                }
                _ => {}
            }
        }
        let gave = !self.powers.is_empty() || !self.cards.is_empty();
        self.random = gave && !same_rng(s, scratch);
    }
}
