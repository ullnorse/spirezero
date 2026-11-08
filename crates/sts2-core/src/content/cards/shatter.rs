//! `Shatter` (`Models.Cards/Shatter.cs`): cost 1, Exhaust, 7 damage (11 upgraded) to all enemies, then evoke every orb twice.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Shatter;

#[sts2_macros::hooks]
impl Hooks for Shatter {
    type Me = CardId;
}

impl CardModel for Shatter {
    fn name(&self) -> &'static str {
        "Shatter"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 7).upgrade(4)];
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
        // The orb count is read once, after the damage; each round is a Dualcast.
        for _ in 0..eng.s.player.orbs.len() {
            eng.evoke_next(false)?;
            eng.evoke_next(true)?;
        }
        Ok(())
    }
}
