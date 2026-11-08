//! Monsters (and Osty, a pet monster): one file per monster, each implementing
//! [`MonsterModel`] (HP, move table, moves) and [`Hooks`](crate::combat::hooks::Hooks).
//!
//! To add a monster: its file `<name>.rs` declaring `pub struct <Name>`; `build.rs` generates the
//! variant (holding the struct, the model's per-combat fields), `id` and `model` (see
//! `docs/specs/content-api.md`, "Registration").

include!(concat!(env!("OUT_DIR"), "/content/monsters.rs"));

pub(crate) mod decimillipede_segment;
pub mod machine;

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Event, Flow, IntentType, PileType};
use crate::content::cards::CardKind;
use crate::content::powers::illusion;
use crate::decimal::Decimal;
use crate::rng::{Rng, RunRngType};

use machine::{ForcedMove, MoveMachine, MoveTable, StateIdx};

/// A monster's static data and moves (`MonsterModel`).
#[expect(
    unused_variables,
    reason = "default bodies ignore their arguments, like the C# virtual methods"
)]
pub(crate) trait MonsterModel: Hooks<Me = CreatureId> {
    /// Whether this model can raise a player choice ([`crate::combat::DecisionRequest`]) from
    /// any of its entry points or hooks, directly or by creating, transforming into or playing a
    /// model that can. The combat snapshots a step only while such a model is in combat, so a
    /// wrong `false` panics at the choice (debug builds also check replay determinism).
    fn may_prompt(&self) -> bool {
        false
    }

    /// The English display name (`<ENTRY>.name` in `localization_eng/monsters.json`).
    fn name(&self) -> &'static str;
    /// `MinInitialHp`.
    fn min_initial_hp(&self, ascension: u8) -> i32;
    /// `MaxInitialHp`; also the HP a creature is constructed with.
    fn max_initial_hp(&self, ascension: u8) -> i32;
    /// `GenerateMoveStateMachine`.
    fn moves(&self) -> &'static MoveTable;
    /// Runs the move of state `state` (a `MoveState` of [`MonsterModel::moves`]) against the
    /// `PlayerCreatures`.
    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow;
    /// The `IntentType` of the attacks of the `MoveState` `state`: `DeathBlow` for a move built
    /// with `DeathBlowIntent` (`GasBomb`, `WaterfallGiant`), else `Attack`.
    fn attack_intent_type(&self, state: StateIdx) -> IntentType {
        IntentType::Attack
    }
    /// `AfterAddedToRoom`.
    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        Ok(())
    }
    /// `BeforeRemovedFromRoom` (sync): when a dead enemy leaves combat
    /// (`CombatManager.RemoveCreature`).
    fn before_removed_from_room(&self, s: &mut CombatState, me: CreatureId) {}
}

impl MonsterKind {
    /// The English display name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        with_model!(self, m => m.name())
    }

    /// Whether the model can raise a player choice (see `may_prompt` in the content API),
    /// statically dispatched: [`CombatState::may_prompt`](crate::combat::CombatState::may_prompt)
    /// asks every model on every step.
    #[must_use]
    pub fn may_prompt(&self) -> bool {
        with_model!(self, m => m.may_prompt())
    }

    /// `MinInitialHp`.
    #[must_use]
    pub fn min_initial_hp(&self, ascension: u8) -> i32 {
        with_model!(self, m => m.min_initial_hp(ascension))
    }

    /// `MaxInitialHp`; also the HP a creature is constructed with.
    #[must_use]
    pub fn max_initial_hp(&self, ascension: u8) -> i32 {
        with_model!(self, m => m.max_initial_hp(ascension))
    }

    /// The move table.
    #[must_use]
    pub fn moves(&self) -> &'static MoveTable {
        with_model!(self, m => m.moves())
    }
}

/// `MoveState.Id` of the `NextMove` a monster has before its first roll.
const UNSET_MOVE: &str = "UNSET_MOVE";

/// A monster attached to a creature: its model plus the generic `MonsterModel` state.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Monster {
    /// The monster model and its per-combat fields.
    pub kind: MonsterKind,
    /// `MoveStateMachine`.
    pub machine: MoveMachine,
    /// `MonsterModel.Rng`, seeded per creature in `CombatState.CreateCreature`.
    pub rng: Rng,
    /// `SpawnedThisTurn`: set by `SetUpForCombat`, cleared on every side switch.
    pub spawned_this_turn: bool,
    /// `IsPerformingMove`.
    pub is_performing_move: bool,
    /// Whether `RollMove` ran: `NextMove` is `UNSET_MOVE` until it does (a creature added
    /// mid-turn is not rolled until the next player turn starts).
    pub rolled: bool,
}

impl_clone_by_fields!(Monster {
    kind,
    machine,
    rng,
    spawned_this_turn,
    is_performing_move,
    rolled,
});

impl Monster {
    /// A freshly set-up monster (`SetUpForCombat`).
    #[must_use]
    pub fn new(kind: MonsterKind, rng: Rng) -> Self {
        Self {
            kind,
            machine: MoveMachine::new(kind.moves()),
            rng,
            spawned_this_turn: true,
            is_performing_move: false,
            rolled: false,
        }
    }

    /// Whether this is Osty.
    #[must_use]
    pub const fn is_osty(&self) -> bool {
        matches!(self.kind, MonsterKind::Osty(_))
    }

    /// `NextMove.Id`: `UNSET_MOVE` before the first roll, unless a move was forced.
    #[must_use]
    pub fn next_move_name(&self) -> &'static str {
        if !self.rolled && self.machine.forced.is_none() {
            return UNSET_MOVE;
        }
        self.machine.current_id(self.kind.moves())
    }
}

/// The monster on creature `id`.
fn monster(state: &CombatState, id: CreatureId) -> &Monster {
    state
        .creature(id)
        .monster
        .as_ref()
        .expect("the creature is a monster")
}

/// `Creature.AfterAddedToRoom` for an enemy: the monster's `AfterAddedToRoom`.
pub(crate) fn after_added_to_room(eng: &mut Engine<'_>, id: CreatureId) -> Flow {
    let kind = monster(eng.s, id).kind;
    kind.model().after_added_to_room(eng, id)
}

/// `MonsterModel.BeforeRemovedFromRoom`.
pub(crate) fn before_removed_from_room(eng: &mut Engine<'_>, id: CreatureId) {
    let kind = monster(eng.s, id).kind;
    kind.model().before_removed_from_room(eng.s, id);
}

/// `MonsterModel.RollMove` on `Rng.MonsterAi` (`MonsterModel.cs:416`).
pub(crate) fn roll_move(eng: &mut Engine<'_>, id: CreatureId) {
    let m = monster(eng.s, id);
    let table = m.kind.moves();
    // `MoveMachine::roll` on the machine in the state, without copying it: the walk only
    // reads the machine (its conditions see the state as before the roll either way).
    let next = m.machine.rolls(table).then(|| {
        let mut rng = eng.s.rng[RunRngType::MonsterAi].clone();
        let next = m.machine.next_move(table, eng.s, id, &mut rng);
        (next, rng)
    });
    let next = next.map(|(next, rng)| {
        eng.s.rng[RunRngType::MonsterAi] = rng;
        next
    });
    if let Some(m) = eng.s.creature_mut(id).monster.as_mut() {
        if let Some(next) = next {
            m.machine.enter(next);
        }
        m.rolled = true;
    }
}

/// `MonsterModel.PerformMove` (`MonsterModel.cs:435`).
pub(crate) fn perform_move(eng: &mut Engine<'_>, id: CreatureId) -> Flow {
    let m = monster(eng.s, id);
    let (kind, state, forced) = (m.kind, m.machine.current, m.machine.forced);
    let name = m.next_move_name();
    set_performing(eng, id, true);
    eng.emit(Event::MonsterMove { monster: id, name });
    if let Some(forced) = forced {
        // MoveState.PerformMove: `_performedAtLeastOnce` is set before the move runs.
        if let Some(f) = eng
            .s
            .creature_mut(id)
            .monster
            .as_mut()
            .and_then(|m| m.machine.forced.as_mut())
        {
            f.performed_at_least_once = true;
        }
        match forced.kind {
            ForcedMove::IllusionRevive => illusion::revive_move(eng, id)?,
            // The wrapper only plays a VFX and runs the caller's move, which is a no-op in scope
            // (`CeremonialBeast.StunnedMove` resets an animation flag).
            ForcedMove::Stunned => {}
            ForcedMove::BeetleWakeUp => SlumberingBeetle::wake_up_move(eng, id)?,
            ForcedMove::AboutToBlow => waterfall_giant::about_to_blow_move(eng, id)?,
            ForcedMove::TestSubjectRespawn => TestSubject::respawn_move(eng, id)?,
        }
    } else {
        kind.model().perform_move(eng, id, state)?;
    }
    if let Some(m) = eng.s.creature_mut(id).monster.as_mut() {
        // MoveStateMachine.OnMovePerformed.
        m.machine.performed_first_move = true;
        m.is_performing_move = false;
    }
    if eng.s.creature(id).is_dead() && eng.hooked().should_creature_be_removed_after_death(id) {
        eng.s.remove_enemy(id);
    }
    Ok(())
}

fn set_performing(eng: &mut Engine<'_>, id: CreatureId, value: bool) {
    if let Some(m) = eng.s.creature_mut(id).monster.as_mut() {
        m.is_performing_move = value;
    }
}

/// Updates the per-combat fields of the monster on `id`.
pub(crate) fn update_kind(eng: &mut Engine<'_>, id: CreatureId, f: impl FnOnce(&mut MonsterKind)) {
    if let Some(m) = eng.s.creature_mut(id).monster.as_mut() {
        f(&mut m.kind);
    }
}

/// `CreatureCmd.Stun(creature, stunMove, nextMoveId)` (`Creature.StunInternal`): replaces the
/// creature's next move with `STUNNED`, which must be performed once and then follows up with
/// `next` (`nextMoveId`), or with the last move logged if `None`. Nothing for a dead creature.
pub(crate) fn stun(eng: &mut Engine<'_>, id: CreatureId, next: Option<StateIdx>) {
    stun_with(eng, id, ForcedMove::Stunned, next);
}

/// [`stun`] with a stun move that does something when performed (`stunMove`).
pub(crate) fn stun_with(
    eng: &mut Engine<'_>,
    id: CreatureId,
    kind: ForcedMove,
    next: Option<StateIdx>,
) {
    if eng.s.creature(id).is_dead() {
        return;
    }
    if let Some(m) = eng.s.creature_mut(id).monster.as_mut() {
        let follow_up = next.unwrap_or_else(|| {
            m.machine
                .log
                .last()
                .expect("a stunned monster has logged a move")
        });
        m.machine.set_move_immediate(kind, follow_up, true);
    }
}

/// `DamageCmd.Attack(damage).FromMonster(this).Execute(null)`: one powered hit on every
/// `PlayerCreatures` entry (`AttackCommand.cs:171-174`).
pub(crate) fn attack_players(eng: &mut Engine<'_>, id: CreatureId, damage: i32) -> Flow {
    attack_players_hits(eng, id, damage, 1)
}

/// `DamageCmd.Attack(damage).WithHitCount(hits).FromMonster(this).Execute(null)`.
pub(crate) fn attack_players_hits(
    eng: &mut Engine<'_>,
    id: CreatureId,
    damage: i32,
    hits: u32,
) -> Flow {
    let mut attack = Attack::new(
        id,
        AttackTargets::PlayerCreatures,
        AttackDamage::Fixed(Decimal::from(damage)),
        None,
    );
    attack.hits = hits;
    eng.attack(&attack)?;
    Ok(())
}

/// `AscensionHelper.GetValueIfAscension(level, then, otherwise)`.
pub(crate) const fn if_ascension(ascension: u8, level: u8, then: i32, otherwise: i32) -> i32 {
    if ascension >= level { then } else { otherwise }
}

/// `AscensionLevel.ToughEnemies`.
pub(crate) const TOUGH_ENEMIES: u8 = 8;
/// `AscensionLevel.DeadlyEnemies`.
pub(crate) const DEADLY_ENEMIES: u8 = 9;

/// `CardPileCmd.AddToCombatAndPreview<Slimed>(targets, Discard, count)` with the move's
/// targets, the `PlayerCreatures`.
pub(crate) fn add_slimed(eng: &mut Engine<'_>, count: u32) -> Flow {
    add_status_cards(eng, CardKind::Slimed, count)
}

/// `CardPileCmd.AddToCombatAndPreview<T>(targets, Discard, count)` with the move's targets, the
/// `PlayerCreatures`.
pub(crate) fn add_status_cards(eng: &mut Engine<'_>, kind: CardKind, count: u32) -> Flow {
    for target in eng.s.player_creatures() {
        eng.add_to_combat_and_preview(target, kind, PileType::Discard, count, None)?;
    }
    Ok(())
}
