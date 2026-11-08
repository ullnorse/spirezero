//! `WaterfallGiant` (`Models.Monsters/WaterfallGiant.cs`): Pressurize (`SteamEruption` for 20, 15 at
//! base) -> Stomp -> Ram -> Siphon -> Pressure Gun -> Pressure Up -> Stomp ..., each move after
//! Pressurize adding 3 `SteamEruption`. When it dies with `SteamEruption`, it does not leave: it
//! comes back at 999999999 HP and is forced into About To Blow (the power turns into the eruption's
//! damage), then Explode, which attacks and kills itself.
//!
//! About To Blow also makes its HP display infinite (read by `HellraiserPower`).
//!
//! The animation tracks (`PressureBuildupIdx`, `IsAboutToBlow`) and the music and ambience
//! parameters are cosmetic; `ShouldDisappearFromDoom` and `ShouldFadeAfterDeath` only pick animations.

use super::machine::{ForcedMove, MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, IntentType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const PRESSURIZE: StateIdx = 0;
const STOMP: StateIdx = 1;
const RAM: StateIdx = 2;
const SIPHON: StateIdx = 3;
const PRESSURE_GUN: StateIdx = 4;
const PRESSURE_UP: StateIdx = 5;
const EXPLODE: StateIdx = 6;

/// `PressureGunIncrease`.
const PRESSURE_GUN_INCREASE: i32 = 5;
/// The HP `TriggerAboutToBlowState` sets.
const ABOUT_TO_BLOW_HP: i32 = 999_999_999;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "PRESSURIZE_MOVE",
            follow_up: STOMP,
        },
        StateDef::Move {
            id: "STOMP_MOVE",
            follow_up: RAM,
        },
        StateDef::Move {
            id: "RAM_MOVE",
            follow_up: SIPHON,
        },
        StateDef::Move {
            id: "SIPHON_MOVE",
            follow_up: PRESSURE_GUN,
        },
        StateDef::Move {
            id: "PRESSURE_GUN_MOVE",
            follow_up: PRESSURE_UP,
        },
        StateDef::Move {
            id: "PRESSURE_UP_MOVE",
            follow_up: STOMP,
        },
        StateDef::Move {
            id: "EXPLODE_MOVE",
            follow_up: EXPLODE,
        },
    ],
    initial: PRESSURIZE,
};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WaterfallGiant {
    /// `_currentPressureGunDamage`, set by `AfterAddedToRoom`.
    pub current_pressure_gun_damage: i32,
    /// `_steamEruptionDamage`, set by About To Blow.
    pub steam_eruption_damage: i32,
}

fn fields(s: &CombatState, id: CreatureId) -> WaterfallGiant {
    match s.creature(id).monster.as_ref().map(|m| m.kind) {
        Some(MonsterKind::WaterfallGiant(g)) => g,
        _ => unreachable!("the owner is a WaterfallGiant"),
    }
}

fn set_fields(eng: &mut Engine<'_>, id: CreatureId, f: impl FnOnce(&mut WaterfallGiant)) {
    super::update_kind(eng, id, |kind| match kind {
        MonsterKind::WaterfallGiant(g) => f(g),
        _ => unreachable!("the owner is a WaterfallGiant"),
    });
}

/// `PowerCmd.Apply<SteamEruptionPower>(Creature, 3m, Creature, null)`, at the end of most moves.
fn build_pressure(eng: &mut Engine<'_>, me: CreatureId) -> Flow {
    eng.apply_power(me, PowerKind::SteamEruption, Decimal::from(3), Some(me))
}

impl WaterfallGiant {
    /// `TriggerAboutToBlowState`, called by `SteamEruptionPower` when the giant dies: back to life
    /// with `SetMaxAndCurrentHp(999999999)` and `SetMoveImmediate(AboutToBlowState,
    /// forceTransition: true)`.
    pub(crate) fn trigger_about_to_blow_state(eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        trigger_about_to_blow_state(eng, me)
    }
}

fn trigger_about_to_blow_state(eng: &mut Engine<'_>, me: CreatureId) -> Flow {
    // CreatureCmd.SetMaxAndCurrentHp: SetMaxHp, then SetCurrentHp (HP hook only if it changed).
    eng.set_max_hp(me, Decimal::from(ABOUT_TO_BLOW_HP))?;
    let old = eng.s.creature(me).hp;
    let creature = eng.s.creature_mut(me);
    creature.hp = ABOUT_TO_BLOW_HP.min(creature.max_hp);
    let new = creature.hp;
    eng.s.recount_enemies();
    if new != old {
        eng.after_current_hp_changed(me, Decimal::from(new - old))?;
    }
    if eng.s.creature(me).is_dead() {
        eng.kill(&[me])?;
    }
    // `HpDisplay.InfiniteWithoutNumbers` (`WaterfallGiant.cs:307`).
    eng.s.creature_mut(me).infinite_hp_display = true;
    if let Some(m) = eng.s.creature_mut(me).monster.as_mut() {
        m.machine
            .force_current_move(ForcedMove::AboutToBlow, EXPLODE, true);
    }
    Ok(())
}

/// `AboutToBlowMove`: the eruption's damage is the power's amount, and the power goes.
pub fn about_to_blow_move(eng: &mut Engine<'_>, me: CreatureId) -> Flow {
    let amount = eng.s.creature(me).power_amount(PowerKind::SteamEruption);
    set_fields(eng, me, |g| g.steam_eruption_damage = amount);
    let power = eng
        .s
        .creature(me)
        .powers
        .iter()
        .find(|p| p.kind == PowerKind::SteamEruption)
        .map(|p| p.id);
    power.map_or(Ok(()), |power| eng.remove_power(me, power))
}

#[sts2_macros::hooks]
impl Hooks for WaterfallGiant {
    type Me = CreatureId;
}

impl MonsterModel for WaterfallGiant {
    fn name(&self) -> &'static str {
        "Waterfall Giant"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 250, 240)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    /// `EXPLODE_MOVE` is built with a `DeathBlowIntent` (`WaterfallGiant.cs:205`).
    fn attack_intent_type(&self, state: StateIdx) -> IntentType {
        if state == EXPLODE {
            IntentType::DeathBlow
        } else {
            IntentType::Attack
        }
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            PRESSURIZE => {
                let amount = if_ascension(ascension, DEADLY_ENEMIES, 20, 15);
                eng.apply_power(
                    me,
                    PowerKind::SteamEruption,
                    Decimal::from(amount),
                    Some(me),
                )
            }
            STOMP => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 15))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::ONE, Some(me))?;
                build_pressure(eng, me)
            }
            RAM => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 11, 10))?;
                build_pressure(eng, me)
            }
            SIPHON => {
                // `SiphonHeal * CombatState.Players.Count`.
                let heal = if_ascension(ascension, TOUGH_ENEMIES, 15, 10);
                let players =
                    i32::try_from(eng.s.player_creatures().len()).expect("a handful of players");
                eng.heal(me, Decimal::from(heal * players))?;
                build_pressure(eng, me)
            }
            PRESSURE_GUN => {
                let damage = fields(eng.s, me).current_pressure_gun_damage;
                attack_players(eng, me, damage)?;
                set_fields(eng, me, |g| {
                    g.current_pressure_gun_damage += PRESSURE_GUN_INCREASE;
                });
                build_pressure(eng, me)
            }
            PRESSURE_UP => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 14, 13))?;
                build_pressure(eng, me)
            }
            EXPLODE => {
                attack_players(eng, me, fields(eng.s, me).steam_eruption_damage)?;
                eng.kill(&[me])
            }
            _ => unreachable!("not a WaterfallGiant move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 23, 20);
        set_fields(eng, me, |g| g.current_pressure_gun_damage = damage);
        Ok(())
    }
}
