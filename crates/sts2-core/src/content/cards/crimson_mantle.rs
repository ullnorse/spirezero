//! `CrimsonMantle` (`Models.Cards/CrimsonMantle.cs`): cost 1, applies `CrimsonMantlePower` (lose HP then gain 7 Block each turn; 10 upgraded) and raises its self damage by 1.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::{PowerKind, crimson_mantle};

pub struct CrimsonMantle;

#[sts2_macros::hooks]
impl Hooks for CrimsonMantle {
    type Me = CardId;
}

impl CardModel for CrimsonMantle {
    fn name(&self) -> &'static str {
        "Crimson Mantle"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] =
            &[CardVar::new(Var::Power(PowerKind::CrimsonMantle), 7).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let power = eng.apply_power_instance_from_card(
            Some(play.card),
            player,
            PowerKind::CrimsonMantle,
            eng.s
                .card(play.card)
                .var(Var::Power(PowerKind::CrimsonMantle)),
            Some(player),
        )?;
        if let Some(power) = power {
            crimson_mantle::increment_self_damage(eng.s, power);
        }
        Ok(())
    }
}
