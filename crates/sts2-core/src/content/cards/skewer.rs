//! `Skewer` (`Models.Cards/Skewer.cs`): cost X, X hits of 8 damage (11 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Skewer;

#[sts2_macros::hooks]
impl Hooks for Skewer {
    type Me = CardId;
}

impl CardModel for Skewer {
    fn name(&self) -> &'static str {
        "Skewer"
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
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 8).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Skewer needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        // A negative X (only from a ModifyXValue listener) means no hits, like the C# loop.
        let hits = u32::try_from(eng.hooked().resolve_energy_x_value(play.card)).unwrap_or(0);
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}
