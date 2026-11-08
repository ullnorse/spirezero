//! `CharonsAshes` (`Models.Relics/CharonsAshes.cs`): whenever the owner exhausts a card, deals 3
//! unpowered damage to every hittable enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DamageVar(3m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 3;

pub struct CharonsAshes;

impl RelicModel for CharonsAshes {
    fn name(&self) -> &'static str {
        "Charon's Ashes"
    }
}

#[sts2_macros::hooks]
impl Hooks for CharonsAshes {
    type Me = RelicId;

    fn after_card_exhausted(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _card: CardId,
        _caused_by_ethereal: bool,
    ) -> Flow {
        // `card.Owner == Owner`: the only player owns every card. Flash is visual only.
        let owner = eng.s.player.creature;
        let enemies = eng.hooked().hittable_enemies();
        eng.damage(
            &enemies,
            Decimal::from(DAMAGE),
            ValueProps::UNPOWERED,
            Some(owner),
            None,
        )?;
        Ok(())
    }
}
