//! `Whirlwind` (`Models.Cards/Whirlwind.cs`): cost X, X hits of 5 damage (8 upgraded) on all enemies.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Whirlwind;

#[sts2_macros::hooks]
impl Hooks for Whirlwind {
    type Me = CardId;
}

impl CardModel for Whirlwind {
    fn name(&self) -> &'static str {
        "Whirlwind"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn has_energy_cost_x(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 5).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let damage = eng.s.card(play.card).var(Var::Damage);
        // A negative X (only from a ModifyXValue listener) means no hits, like the C# loop.
        let hits = u32::try_from(eng.hooked().resolve_energy_x_value(play.card)).unwrap_or(0);
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::AllOpponents,
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}
