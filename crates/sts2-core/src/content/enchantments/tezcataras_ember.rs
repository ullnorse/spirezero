//! `TezcatarasEmber` (`Models.Enchantments/TezcatarasEmber.cs`): `OnEnchant` makes the card cost 0
//! (`EnergyCost.UpgradeBy(-GetWithModifiers(None))`, i.e. minus its base) and adds Eternal;
//! powered attack damage of the card is 3 higher (`DamageVar(3, Move)`).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, ValueProps};
use crate::content::cards::CardKeyword;
use crate::content::enchantments::{Enchantment, EnchantmentModel};
use crate::decimal::Decimal;

/// `DamageVar(3m, ValueProp.Move)`.
pub const DAMAGE: Decimal = Decimal::new(3, 0);

/// Costs 0, Eternal, +3 attack damage.
pub struct TezcatarasEmber;

impl EnchantmentModel for TezcatarasEmber {
    fn name(&self) -> &'static str {
        "Tezcatara's Ember"
    }

    fn on_enchant(&self, card: &mut Card) {
        let base = card.energy_cost.base;
        card.energy_cost.upgrade_by(-base);
        card.keywords.insert(CardKeyword::Eternal);
    }

    fn enchant_damage_additive(
        &self,
        _enchantment: Enchantment,
        _damage: Decimal,
        props: ValueProps,
    ) -> Decimal {
        if !props.is_powered_attack() {
            return Decimal::ZERO;
        }
        DAMAGE
    }
}

#[sts2_macros::hooks]
impl Hooks for TezcatarasEmber {
    type Me = CardId;
}
