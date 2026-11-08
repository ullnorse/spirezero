//! `Haze` (`Models.Cards/Haze.cs`): cost 2, 4 Poison (6 upgraded) then 1 Weak (2 upgraded) to every hittable enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Haze;

#[sts2_macros::hooks]
impl Hooks for Haze {
    type Me = CardId;
}

impl CardModel for Haze {
    fn name(&self) -> &'static str {
        "Haze"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Poison), 4).upgrade(2),
            CardVar::new(Var::Power(PowerKind::Weak), 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let poison = eng.s.card(play.card).var(Var::Power(PowerKind::Poison));
        let weak = eng.s.card(play.card).var(Var::Power(PowerKind::Weak));
        let player = eng.s.player.creature;
        let enemies = eng.hooked().hittable_enemies();
        eng.apply_power_to_all_from_card(
            Some(play.card),
            &enemies,
            PowerKind::Poison,
            poison,
            Some(player),
        )?;
        let enemies = eng.hooked().hittable_enemies();
        eng.apply_power_to_all_from_card(
            Some(play.card),
            &enemies,
            PowerKind::Weak,
            weak,
            Some(player),
        )
    }
}
