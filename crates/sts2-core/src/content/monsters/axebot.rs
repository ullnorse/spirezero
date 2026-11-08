//! `Axebot` (`Models.Monsters/Axebot.cs`): Hammer Uppercut (damage, 2 Weak, 2 Frail) <-> One-Two
//! (2 hits), after a Boot Up (Block and Strength) when it is a respawn. It starts with 2 `Stock`;
//! each death respawns it in its slot with one stock less, 10 more max HP and 3 more Strength
//! per Boot Up for each respawn so far.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const BOOT_UP: StateIdx = 0;
const ONE_TWO: StateIdx = 1;
const HAMMER_UPPERCUT: StateIdx = 2;

const BOOT_UP_MOVE: StateDef = StateDef::Move {
    id: "BOOT_UP_MOVE",
    follow_up: HAMMER_UPPERCUT,
};
const ONE_TWO_MOVE: StateDef = StateDef::Move {
    id: "ONE_TWO_MOVE",
    follow_up: HAMMER_UPPERCUT,
};
const HAMMER_UPPERCUT_MOVE: StateDef = StateDef::Move {
    id: "HAMMER_UPPERCUT_MOVE",
    follow_up: ONE_TWO,
};

/// A fresh Axebot starts at Hammer Uppercut.
static MOVES: MoveTable = MoveTable {
    states: &[BOOT_UP_MOVE, ONE_TWO_MOVE, HAMMER_UPPERCUT_MOVE],
    initial: HAMMER_UPPERCUT,
};

/// A respawn (`_stockOverrideAmount` set) starts at Boot Up.
static MOVES_RESPAWN: MoveTable = MoveTable {
    states: &[BOOT_UP_MOVE, ONE_TWO_MOVE, HAMMER_UPPERCUT_MOVE],
    initial: BOOT_UP,
};

/// `_initialStockAmount`.
const INITIAL_STOCK: i32 = 2;
/// `_respawnMaxHpBonus`.
const RESPAWN_MAX_HP_BONUS: i32 = 10;

/// Per-combat fields (`StockPower` sets the override when it respawns the monster).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Axebot {
    /// `_stockOverrideAmount`.
    pub stock_override: Option<i32>,
}

impl Axebot {
    /// `StockAmount`.
    const fn stock_amount(self) -> i32 {
        match self.stock_override {
            Some(n) => n,
            None => INITIAL_STOCK,
        }
    }

    /// `RespawnCount`.
    const fn respawn_count(self) -> i32 {
        INITIAL_STOCK - self.stock_amount()
    }

    /// `RespawnMaxHpBonus`.
    const fn respawn_max_hp_bonus(self) -> i32 {
        self.respawn_count() * RESPAWN_MAX_HP_BONUS
    }
}

#[sts2_macros::hooks]
impl Hooks for Axebot {
    type Me = CreatureId;
}

impl MonsterModel for Axebot {
    fn name(&self) -> &'static str {
        "Axebot"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 76, 70) + self.respawn_max_hp_bonus()
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 86, 78) + self.respawn_max_hp_bonus()
    }

    fn moves(&self) -> &'static MoveTable {
        if self.stock_override.is_some() {
            &MOVES_RESPAWN
        } else {
            &MOVES
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        if self.stock_amount() > 0 {
            eng.apply_power(
                me,
                PowerKind::Stock,
                Decimal::from(self.stock_amount()),
                None,
            )?;
        }
        Ok(())
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            BOOT_UP => {
                let block = if_ascension(ascension, DEADLY_ENEMIES, 15, 10);
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 4, 3);
                eng.gain_block(me, Decimal::from(block), ValueProps::MOVE, None)?;
                eng.apply_power(
                    me,
                    PowerKind::Strength,
                    Decimal::from(strength * self.respawn_count()),
                    Some(me),
                )
            }
            ONE_TWO => {
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 11, 10), 2)
            }
            HAMMER_UPPERCUT => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 18, 14))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::from(2), Some(me))?;
                eng.apply_power_to_all(&targets, PowerKind::Frail, Decimal::from(2), Some(me))
            }
            _ => unreachable!("not an Axebot move: {state}"),
        }
    }
}
