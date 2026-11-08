//! The `Terminal` modifier.
//!
//! `Terminal` (`Models.Modifiers/Terminal.cs`): entering the run's first room (Neow's) costs 1
//! max HP (unless HP and max HP are both 1 already), and every combat room gives the player 5
//! `Plating` (`AfterRoomEntered`; the combat applies it, [`crate::combat::Engine::after_room_entered`]).

use crate::run::RunState;

/// The `PlatingPower` amount.
pub const PLATING: i32 = 5;

/// `AfterRoomEntered` for `RunState.BaseRoom`.
pub(super) fn after_base_room_entered(s: &mut RunState) {
    if s.hp > 1 || s.max_hp > 1 {
        s.lose_max_hp(1);
    }
}
