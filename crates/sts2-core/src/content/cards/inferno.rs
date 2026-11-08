//! `Inferno` (`Models.Cards/Inferno.cs`): cost 1, applies `InfernoPower` (6 damage to every enemy per HP lost on your turn; 9 upgraded) and raises its per-turn self damage by 1.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::{PowerKind, inferno};

pub struct Inferno;

#[sts2_macros::hooks]
impl Hooks for Inferno {
    type Me = CardId;
}

impl CardModel for Inferno {
    fn name(&self) -> &'static str {
        "Inferno"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Inferno), 6).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let power = eng.apply_power_instance_from_card(
            Some(play.card),
            player,
            PowerKind::Inferno,
            eng.s.card(play.card).var(Var::Power(PowerKind::Inferno)),
            Some(player),
        )?;
        if let Some(power) = power {
            inferno::increment_self_damage(eng.s, power);
        }
        Ok(())
    }
}
