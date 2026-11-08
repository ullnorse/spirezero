//! `Inky` (`Models.Enchantments/Inky.cs`): each play of the card also applies 1 Weak
//! (`PowerVar<WeakPower>(1m)`, independent of `Amount`) to its target, or to every hittable
//! enemy when the card targets all enemies.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Creatures, Engine, Flow};
use crate::content::cards::{self, TargetType};
use crate::content::enchantments::EnchantmentModel;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// Applies Weak to the card's target(s) when played.
pub struct Inky;

impl EnchantmentModel for Inky {
    fn name(&self) -> &'static str {
        "Inky"
    }

    fn on_play(&self, eng: &mut Engine<'_>, card: CardId, play: &CardPlay) -> Flow {
        let targets = if cards::target_type(eng.s, card) == TargetType::AllEnemies {
            eng.hooked().hittable_enemies()
        } else {
            Creatures::from_slice(&[play.target.expect("Inky needs a target")])
        };
        let player = eng.s.player.creature;
        eng.apply_power_to_all_from_card(
            Some(card),
            &targets,
            PowerKind::Weak,
            Decimal::ONE,
            Some(player),
        )
    }
}

#[sts2_macros::hooks]
impl Hooks for Inky {
    type Me = CardId;
}
