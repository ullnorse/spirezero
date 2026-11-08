//! `TestSubject` (`Models.Monsters/TestSubject.cs`): three forms. It starts with `Adaptable` and
//! `Enrage` (Strength after each Skill); form one cycles Bite -> Skull Bash (attack, 1
//! Vulnerable). Each death of a form with `Adaptable` forces its dead state (`RESPAWN_MOVE`,
//! performed once, `ForcedMove::TestSubjectRespawn`): the next form revives with its own max HP,
//! the second with `PainfulStabs`, then repeats Multi Claw (hits grow by one per use); the third
//! loses `Adaptable` and `PainfulStabs`, gains `Nemesis`, and cycles Lacerate (three hits) ->
//! Big Pounce -> Burning Growl (`Burn`s into the discard pile, Strength). Its death then ends the
//! fight. `TestSubjectKills` (a run and profile counter that only names it), the colors, the
//! music and the animations are visual.

use super::machine::{ForcedMove, MoveTable, StateDef, StateIdx, when};
use super::{DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, update_kind};
use super::{add_status_cards, attack_players, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerKind, adaptable};
use crate::decimal::Decimal;

const BITE: StateIdx = 0;
const SKULL_BASH: StateIdx = 1;
const MULTI_CLAW: StateIdx = 2;
const PHASE3_LACERATE: StateIdx = 3;
const BIG_POUNCE: StateIdx = 4;
const BURNING_GROWL: StateIdx = 5;
const REVIVE_BRANCH: StateIdx = 6;

/// `BaseMultiClawCount`.
const BASE_MULTI_CLAW_COUNT: u32 = 3;
/// `_phase3LacerateRepeat`.
const PHASE3_LACERATE_REPEAT: u32 = 3;
/// `BigPounceDamage`.
const BIG_POUNCE_DAMAGE: i32 = 45;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "BITE_MOVE",
            follow_up: SKULL_BASH,
        },
        StateDef::Move {
            id: "SKULL_BASH_MOVE",
            follow_up: BITE,
        },
        StateDef::Move {
            id: "MULTI_CLAW_MOVE",
            follow_up: MULTI_CLAW,
        },
        StateDef::Move {
            id: "PHASE3_LACERATE_MOVE",
            follow_up: BIG_POUNCE,
        },
        StateDef::Move {
            id: "BIG_POUNCE",
            follow_up: BURNING_GROWL,
        },
        StateDef::Move {
            id: "BURNING_GROWL_MOVE",
            follow_up: PHASE3_LACERATE,
        },
        StateDef::Conditional {
            id: "REVIVE_BRANCH",
            branches: &[
                when(MULTI_CLAW, fewer_than_two_respawns),
                when(PHASE3_LACERATE, two_respawns),
            ],
        },
    ],
    initial: BITE,
};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TestSubject {
    /// `_respawns`.
    pub respawns: u8,
    /// `_extraMultiClawCount`.
    pub extra_multi_claw_count: u32,
}

fn fields(s: &CombatState, id: CreatureId) -> TestSubject {
    match s.creature(id).monster.as_ref().map(|m| m.kind) {
        Some(MonsterKind::TestSubject(t)) => t,
        _ => unreachable!("the owner is a TestSubject"),
    }
}

fn update(eng: &mut Engine<'_>, id: CreatureId, f: impl FnOnce(&mut TestSubject)) {
    update_kind(eng, id, |kind| match kind {
        MonsterKind::TestSubject(t) => f(t),
        _ => unreachable!("the owner is a TestSubject"),
    });
}

fn fewer_than_two_respawns(s: &CombatState, id: CreatureId) -> bool {
    fields(s, id).respawns < 2
}

fn two_respawns(s: &CombatState, id: CreatureId) -> bool {
    fields(s, id).respawns >= 2
}

/// `PowerCmd.Remove<T>(creature)`: removes the creature's power of `kind`, if any.
fn remove_power_kind(eng: &mut Engine<'_>, owner: CreatureId, kind: PowerKind) -> Flow {
    let power = eng
        .s
        .creature(owner)
        .powers
        .iter()
        .find(|p| p.kind == kind)
        .map(|p| p.id);
    power.map_or(Ok(()), |id| eng.remove_power(owner, id))
}

impl TestSubject {
    /// `TriggerDeadState`, called by `AdaptablePower` when the owner dies:
    /// `SetMoveImmediate(DeadState, forceTransition: true)` (`TestSubjectKills` is not modelled).
    pub(crate) fn trigger_dead_state(eng: &mut Engine<'_>, me: CreatureId) {
        if let Some(m) = eng.s.creature_mut(me).monster.as_mut() {
            m.machine
                .force_current_move(ForcedMove::TestSubjectRespawn, REVIVE_BRANCH, true);
        }
    }

    /// `RespawnMove`, the dead state (`MustPerformOnceBeforeTransitioning`).
    pub(crate) fn respawn_move(eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        update(eng, me, |t| t.respawns += 1);
        adaptable::do_revive(eng.s, me);
        let ascension = eng.s.ascension;
        match fields(eng.s, me).respawns {
            1 => {
                revive(eng, me, if_ascension(ascension, TOUGH_ENEMIES, 212, 200))?;
                eng.apply_power(me, PowerKind::PainfulStabs, Decimal::ONE, Some(me))
            }
            2 => {
                revive(eng, me, if_ascension(ascension, TOUGH_ENEMIES, 313, 300))?;
                eng.apply_power(me, PowerKind::Nemesis, Decimal::ONE, Some(me))?;
                remove_power_kind(eng, me, PowerKind::Adaptable)?;
                remove_power_kind(eng, me, PowerKind::PainfulStabs)
            }
            _ => Ok(()),
        }
    }
}

/// `Revive(baseRespawnHp)`: `ScaleHpForMultiplayer` keeps the HP for one player; `SetMaxHp`,
/// then `Heal` by it.
fn revive(eng: &mut Engine<'_>, me: CreatureId, hp: i32) -> Flow {
    let hp = Decimal::from(hp);
    eng.set_max_hp(me, hp)?;
    eng.heal(me, hp)
}

#[sts2_macros::hooks]
impl Hooks for TestSubject {
    type Me = CreatureId;
}

impl MonsterModel for TestSubject {
    fn name(&self) -> &'static str {
        "Test Subject"
    }

    /// `FirstFormHp`.
    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 111, 100)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let enrage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 3, 2);
        eng.apply_power(me, PowerKind::Adaptable, Decimal::ONE, Some(me))?;
        eng.apply_power(me, PowerKind::Enrage, Decimal::from(enrage), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            BITE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 22, 20)),
            SKULL_BASH => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 14))?;
                for target in eng.s.player_creatures() {
                    eng.apply_power(target, PowerKind::Vulnerable, Decimal::ONE, Some(me))?;
                }
                Ok(())
            }
            MULTI_CLAW => {
                let damage = if_ascension(ascension, DEADLY_ENEMIES, 11, 10);
                let hits = BASE_MULTI_CLAW_COUNT + fields(eng.s, me).extra_multi_claw_count;
                attack_players_hits(eng, me, damage, hits)?;
                update(eng, me, |t| t.extra_multi_claw_count += 1);
                Ok(())
            }
            PHASE3_LACERATE => {
                let damage = if_ascension(ascension, DEADLY_ENEMIES, 11, 10);
                attack_players_hits(eng, me, damage, PHASE3_LACERATE_REPEAT)
            }
            BIG_POUNCE => attack_players(eng, me, BIG_POUNCE_DAMAGE),
            BURNING_GROWL => {
                let burns = if_ascension(ascension, DEADLY_ENEMIES, 5, 3);
                add_status_cards(eng, CardKind::Burn, u32::try_from(burns).unwrap_or(0))?;
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 3, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            _ => unreachable!("not a TestSubject move: {state}"),
        }
    }
}
