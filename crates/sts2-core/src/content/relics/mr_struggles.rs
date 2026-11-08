//! `MrStruggles` (`Models.Relics/MrStruggles.cs`): at the start of each of the owner's turns, as
//! much unpowered damage as the turn number to every hittable enemy. Event rarity (`DollRoom`).

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

pub struct MrStruggles;

impl RelicModel for MrStruggles {
    fn name(&self) -> &'static str {
        "Mr. Struggles"
    }
}

#[sts2_macros::hooks]
impl Hooks for MrStruggles {
    type Me = RelicId;

    /// `AfterPlayerTurnStart`: `CreatureCmd.Damage(HittableEnemies, TurnNumber, Unpowered,
    /// Owner.Creature)`.
    fn after_player_turn_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.player.creature;
        let targets = eng.hooked().hittable_enemies();
        let amount = Decimal::from(i64::from(eng.s.player.turn_number));
        eng.damage(&targets, amount, ValueProps::UNPOWERED, Some(owner), None)?;
        Ok(())
    }
}
