//! `Conflagration` (`Models.Cards/Conflagration.cs`): cost 1, 4 hits (5 upgraded) of 2 damage on all enemies.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Conflagration;

#[sts2_macros::hooks]
impl Hooks for Conflagration {
    type Me = CardId;
}

impl CardModel for Conflagration {
    fn name(&self) -> &'static str {
        "Conflagration"
    }

    fn cost(&self) -> i32 {
        1
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 2),
            CardVar::new(Var::Repeat, 4).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let damage = eng.s.card(play.card).var(Var::Damage);
        let hits = eng.s.card(play.card).count_var(Var::Repeat);
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
