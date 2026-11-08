//! `Deathbringer` (`Models.Cards/Deathbringer.cs`): cost 2; 21 Doom (26 upgraded) and 1 Weak to all enemies.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Deathbringer;

#[sts2_macros::hooks]
impl Hooks for Deathbringer {
    type Me = CardId;
}

impl CardModel for Deathbringer {
    fn name(&self) -> &'static str {
        "Deathbringer"
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
            CardVar::new(Var::Power(PowerKind::Doom), 21).upgrade(5),
            CardVar::new(Var::Power(PowerKind::Weak), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let doom = eng.s.card(play.card).var(Var::Power(PowerKind::Doom));
        let player = eng.s.player.creature;
        let targets = eng.hooked().hittable_enemies();
        eng.apply_power_to_all_from_card(
            Some(play.card),
            &targets,
            PowerKind::Doom,
            doom,
            Some(player),
        )?;
        let targets = eng.hooked().hittable_enemies();
        eng.apply_power_to_all_from_card(
            Some(play.card),
            &targets,
            PowerKind::Weak,
            eng.s.card(play.card).var(Var::Power(PowerKind::Weak)),
            Some(player),
        )
    }
}
