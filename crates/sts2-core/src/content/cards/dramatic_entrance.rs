//! `DramaticEntrance` (`Models.Cards/DramaticEntrance.cs`): cost 0, Exhaust, Innate; 11 damage
//! (15 upgraded) to all enemies.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct DramaticEntrance;

#[sts2_macros::hooks]
impl Hooks for DramaticEntrance {
    type Me = CardId;
}

impl CardModel for DramaticEntrance {
    fn name(&self) -> &'static str {
        "Dramatic Entrance"
    }

    fn cost(&self) -> i32 {
        0
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE
            .with(CardKeyword::Exhaust)
            .with(CardKeyword::Innate)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 11).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
