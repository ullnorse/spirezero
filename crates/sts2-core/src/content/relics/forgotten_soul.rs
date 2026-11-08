//! `ForgottenSoul` (`Models.Relics/ForgottenSoul.cs`): whenever the owner exhausts a card, 1
//! unpowered damage to a random hittable enemy (`Rng.CombatTargets`). Event rarity
//! (`GraveOfTheForgotten`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, Engine, Flow, RelicId, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::rng::RunRngType;

/// `DamageVar(1m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 1;

pub struct ForgottenSoul;

impl RelicModel for ForgottenSoul {
    fn name(&self) -> &'static str {
        "Forgotten Soul"
    }
}

#[sts2_macros::hooks]
impl Hooks for ForgottenSoul {
    type Me = RelicId;

    fn after_card_exhausted(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _card: CardId,
        _caused_by_ethereal: bool,
    ) -> Flow {
        // `card.Owner == Owner`: the only player owns every card.
        let owner = eng.s.player.creature;
        let enemies = eng.hooked().hittable_enemies();
        let target = eng.s.rng[RunRngType::CombatTargets]
            .next_item(&enemies)
            .copied();
        if let Some(target) = target {
            eng.damage(
                &[target],
                Decimal::from(DAMAGE),
                ValueProps::UNPOWERED,
                Some(owner),
                None,
            )?;
        }
        Ok(())
    }
}
