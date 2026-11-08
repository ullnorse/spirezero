//! The `Murderous` modifier.
//!
//! `Murderous` (`Models.Modifiers/Murderous.cs`): `AfterRoomEntered` of a combat room gives every
//! creature in it 3 Strength, and `AfterCreatureAddedToCombat` gives each enemy added later (a
//! summon) 3 as well. The combat applies it ([`crate::combat::Engine::after_room_entered`]).

/// `_strengthAmount`.
pub const STRENGTH: i32 = 3;
