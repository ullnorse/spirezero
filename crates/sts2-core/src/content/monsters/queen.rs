//! `Queen` (`Models.Monsters/Queen.cs`), with her `TorchHeadAmalgam`: Puppet Strings (3
//! `ChainsOfBinding` on each target) -> You're Mine (99 Frail, Weak and Vulnerable) -> Burn
//! Bright for Me (1 Strength to every other teammate, 20 Block) repeated while the amalgam lives;
//! once it died, Off with Your Head (five hits) -> Execution (attack) -> Enrage (2 Strength) ->
//! Off with Your Head... When the amalgam dies while Burn Bright for Me is the next move, Enrage
//! replaces it at once. The talk lines, the music and the animations are visual; `Amalgam` is
//! only read for nothing that matters.

use super::machine::{MoveTable, StateDef, StateIdx, when};
use super::{DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, update_kind};
use super::{attack_players, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const PUPPET_STRINGS: StateIdx = 0;
const YOU_ARE_MINE: StateIdx = 1;
const BURN_BRIGHT_FOR_ME: StateIdx = 2;
const BURN_BRIGHT_FOR_ME_BRANCH: StateIdx = 3;
const YOURE_MINE_NOW_BRANCH: StateIdx = 4;
const OFF_WITH_YOUR_HEAD: StateIdx = 5;
const EXECUTION: StateIdx = 6;
const ENRAGE: StateIdx = 7;

/// `_offWithYourHeadRepeat`.
const OFF_WITH_YOUR_HEAD_REPEAT: u32 = 5;
/// Puppet Strings' `ChainsOfBindingPower` amount.
const CHAINS_OF_BINDING: i32 = 3;
/// You're Mine's Frail, Weak and Vulnerable amount.
const YOU_ARE_MINE_DEBUFF: i32 = 99;
/// Burn Bright for Me's Block.
const BURN_BRIGHT_BLOCK: i32 = 20;
/// Enrage's Strength.
const ENRAGE_STRENGTH: i32 = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "PUPPET_STRINGS_MOVE",
            follow_up: YOU_ARE_MINE,
        },
        StateDef::Move {
            id: "YOU_ARE_MINE_MOVE",
            follow_up: YOURE_MINE_NOW_BRANCH,
        },
        StateDef::Move {
            id: "BURN_BRIGHT_FOR_ME_MOVE",
            follow_up: BURN_BRIGHT_FOR_ME_BRANCH,
        },
        StateDef::Conditional {
            id: "BURN_BRIGHT_FOR_ME_BRANCH",
            branches: &[
                when(BURN_BRIGHT_FOR_ME, amalgam_alive),
                when(OFF_WITH_YOUR_HEAD, amalgam_died),
            ],
        },
        StateDef::Conditional {
            id: "YOURE_MINE_NOW_BRANCH",
            branches: &[
                when(BURN_BRIGHT_FOR_ME, amalgam_alive),
                when(OFF_WITH_YOUR_HEAD, amalgam_died),
            ],
        },
        StateDef::Move {
            id: "OFF_WITH_YOUR_HEAD_MOVE",
            follow_up: EXECUTION,
        },
        StateDef::Move {
            id: "EXECUTION_MOVE",
            follow_up: ENRAGE,
        },
        StateDef::Move {
            id: "ENRAGE_MOVE",
            follow_up: OFF_WITH_YOUR_HEAD,
        },
    ],
    initial: PUPPET_STRINGS,
};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Queen {
    /// `_hasAmalgamDied`.
    pub has_amalgam_died: bool,
}

fn amalgam_died(s: &CombatState, id: CreatureId) -> bool {
    match s.creature(id).monster.as_ref().map(|m| m.kind) {
        Some(MonsterKind::Queen(q)) => q.has_amalgam_died,
        _ => unreachable!("the owner is a Queen"),
    }
}

fn amalgam_alive(s: &CombatState, id: CreatureId) -> bool {
    !amalgam_died(s, id)
}

fn is_amalgam(s: &CombatState, id: CreatureId) -> bool {
    matches!(
        s.creature(id).monster.as_ref().map(|m| m.kind),
        Some(MonsterKind::TorchHeadAmalgam(_))
    )
}

#[sts2_macros::hooks]
impl Hooks for Queen {
    type Me = CreatureId;

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: CreatureId,
        creature: CreatureId,
        _was_removal_prevented: bool,
    ) -> Flow {
        if !is_amalgam(eng.s, creature) || !eng.s.creature(me).is_alive() {
            return Ok(());
        }
        update_kind(eng, me, |kind| match kind {
            MonsterKind::Queen(q) => q.has_amalgam_died = true,
            _ => unreachable!("the owner is a Queen"),
        });
        // `if (NextMove == BurnBrightForMeState) SetMoveImmediate(EnragedState)`: a table move
        // can always transition away; `ForceCurrentState` does not log it.
        if let Some(m) = eng.s.creature_mut(me).monster.as_mut()
            && m.machine.forced.is_none()
            && m.machine.current == BURN_BRIGHT_FOR_ME
            && m.machine.can_transition_away()
        {
            m.machine.current = ENRAGE;
            m.rolled = true;
        }
        Ok(())
    }
}

impl MonsterModel for Queen {
    fn name(&self) -> &'static str {
        "Queen"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 419, 400)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            PUPPET_STRINGS => {
                for target in eng.s.player_creatures() {
                    eng.apply_power(
                        target,
                        PowerKind::ChainsOfBinding,
                        Decimal::from(CHAINS_OF_BINDING),
                        Some(me),
                    )?;
                }
                Ok(())
            }
            YOU_ARE_MINE => {
                let targets = eng.s.player_creatures();
                let amount = Decimal::from(YOU_ARE_MINE_DEBUFF);
                for kind in [PowerKind::Frail, PowerKind::Weak, PowerKind::Vulnerable] {
                    for &target in &targets {
                        eng.apply_power(target, kind, amount, Some(me))?;
                    }
                }
                Ok(())
            }
            BURN_BRIGHT_FOR_ME => {
                // `GetValueIfAscension(DeadlyEnemies, 1, 1)`.
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 1, 1);
                let side = eng.s.creature(me).side;
                for teammate in eng.s.creatures_on(side) {
                    if teammate != me {
                        eng.apply_power(
                            teammate,
                            PowerKind::Strength,
                            Decimal::from(strength),
                            Some(me),
                        )?;
                    }
                }
                eng.gain_block(me, Decimal::from(BURN_BRIGHT_BLOCK), ValueProps::MOVE, None)?;
                Ok(())
            }
            OFF_WITH_YOUR_HEAD => {
                let damage = if_ascension(ascension, DEADLY_ENEMIES, 4, 3);
                attack_players_hits(eng, me, damage, OFF_WITH_YOUR_HEAD_REPEAT)
            }
            EXECUTION => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 18, 15)),
            ENRAGE => eng.apply_power(
                me,
                PowerKind::Strength,
                Decimal::from(ENRAGE_STRENGTH),
                Some(me),
            ),
            _ => unreachable!("not a Queen move: {state}"),
        }
    }
}
