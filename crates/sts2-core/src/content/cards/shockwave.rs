//! `Shockwave` (`Models.Cards/Shockwave.cs`): cost 2, Exhaust, all enemies; apply 3 Weak (5
//! upgraded) then 3 Vulnerable (5 upgraded) to each hittable enemy in turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Shockwave;

#[sts2_macros::hooks]
impl Hooks for Shockwave {
    type Me = CardId;
}

impl CardModel for Shockwave {
    fn name(&self) -> &'static str {
        "Shockwave"
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Power"), 3).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Named("Power"));
        let player = eng.s.player.creature;
        for enemy in eng.hooked().hittable_enemies() {
            eng.apply_power_from_card(
                Some(play.card),
                enemy,
                PowerKind::Weak,
                amount,
                Some(player),
            )?;
            eng.apply_power_from_card(
                Some(play.card),
                enemy,
                PowerKind::Vulnerable,
                amount,
                Some(player),
            )?;
        }
        Ok(())
    }
}
