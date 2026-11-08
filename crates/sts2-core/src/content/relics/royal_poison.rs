//! `RoyalPoison` (`Models.Relics/RoyalPoison.cs`): 4 unblockable unpowered damage to the owner at
//! the start of the first turn of each combat. Event rarity (`RoundTeaParty`).

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DamageVar(4m, ValueProp.Unblockable | ValueProp.Unpowered)`.
const DAMAGE: i32 = 4;

pub struct RoyalPoison;

impl RelicModel for RoyalPoison {
    fn name(&self) -> &'static str {
        "Royal Poison"
    }
}

#[sts2_macros::hooks]
impl Hooks for RoyalPoison {
    type Me = RelicId;

    /// `player == Owner && TurnNumber <= 1`; `CreatureCmd.Damage(.., Owner.Creature, Damage, null,
    /// null)`.
    fn after_player_turn_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.player.turn_number > 1 {
            return Ok(());
        }
        let owner = eng.s.player.creature;
        eng.damage(
            &[owner],
            Decimal::from(DAMAGE),
            ValueProps::UNBLOCKABLE.with(ValueProps::UNPOWERED),
            None,
            None,
        )?;
        Ok(())
    }
}
