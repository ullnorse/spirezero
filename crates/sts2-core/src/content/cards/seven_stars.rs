//! `SevenStars` (`Models.Cards/SevenStars.cs`): Regent rare attack, cost 2 (1 upgraded) and 7 stars; deal 7 damage to all enemies 7 times.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct SevenStars;

#[sts2_macros::hooks]
impl Hooks for SevenStars {
    type Me = CardId;
}

impl CardModel for SevenStars {
    fn name(&self) -> &'static str {
        "Seven Stars"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn star_cost(&self) -> i32 {
        7
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

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 7), CardVar::new(Var::Repeat, 7)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::AllOpponents,
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(eng.s.card(play.card).count_var(Var::Repeat)),
        )?;
        Ok(())
    }
}
