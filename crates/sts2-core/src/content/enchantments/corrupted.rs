//! `Corrupted` (`Models.Enchantments/Corrupted.cs`): attacks only; ×1.5 powered attack damage,
//! and every play costs the owner 2 HP (`Symbiote`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::CardType;
use crate::content::enchantments::{Enchantment, EnchantmentModel};
use crate::decimal::Decimal;

/// `_damageAmount`.
const HP_LOSS: i32 = 2;

/// ×1.5 damage, 2 HP lost per play.
pub struct Corrupted;

impl EnchantmentModel for Corrupted {
    fn name(&self) -> &'static str {
        "Corrupted"
    }

    fn can_enchant_card_type(&self, card_type: CardType) -> bool {
        card_type == CardType::Attack
    }

    fn enchant_damage_multiplicative(
        &self,
        _enchantment: Enchantment,
        _damage: Decimal,
        props: ValueProps,
    ) -> Decimal {
        if props.is_powered_attack() {
            Decimal::new(15, 1)
        } else {
            Decimal::ONE
        }
    }

    /// `OnPlay`: `CreatureCmd.Damage(Card.Owner.Creature, 2m, Unblockable | Unpowered | Move,
    /// Card, cardPlay)`; the dealer is the card's owner.
    fn on_play(&self, eng: &mut Engine<'_>, card: CardId, _play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.damage(
            &[player],
            Decimal::from(HP_LOSS),
            ValueProps::UNBLOCKABLE
                .with(ValueProps::UNPOWERED)
                .with(ValueProps::MOVE),
            Some(player),
            Some(card),
        )?;
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Corrupted {
    type Me = CardId;
}
