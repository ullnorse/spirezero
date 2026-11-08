//! `DyingStar` (`Models.Cards/DyingStar.cs`): Regent rare attack, cost 1 and 3 stars, Ethereal,
//! all enemies; deal 9 damage (11 upgraded) to every enemy, then apply `DyingStarPower` (a
//! temporary Strength loss of 9, 11 upgraded) to every enemy that was hittable before the attack.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct DyingStar;

#[sts2_macros::hooks]
impl Hooks for DyingStar {
    type Me = CardId;
}

impl CardModel for DyingStar {
    fn name(&self) -> &'static str {
        "Dying Star"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn star_cost(&self) -> i32 {
        3
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Ethereal)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 9).upgrade(2),
            CardVar::new(Var::Named("StrengthLoss"), 9).upgrade(2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let card = eng.s.card(play.card);
        let damage = card.var(Var::Damage);
        let loss = card.var(Var::Named("StrengthLoss"));
        let enemies = eng.hooked().hittable_enemies();
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.apply_power_to_all_from_card(
            Some(play.card),
            &enemies,
            PowerKind::DyingStar,
            loss,
            Some(player),
        )
    }
}
