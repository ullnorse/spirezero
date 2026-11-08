//! `CrushUnder` (`Models.Cards/CrushUnder.cs`): Regent common attack, cost 1, all enemies; deal
//! 8 damage (9 upgraded) to every enemy, then apply `CrushUnderPower` (a temporary Strength
//! loss of 1, 2 upgraded) to every enemy that was hittable before the attack.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct CrushUnder;

#[sts2_macros::hooks]
impl Hooks for CrushUnder {
    type Me = CardId;
}

impl CardModel for CrushUnder {
    fn name(&self) -> &'static str {
        "Crush Under"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 8).upgrade(1),
            CardVar::new(Var::Named("StrengthLoss"), 1).upgrade(1),
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
            PowerKind::CrushUnder,
            loss,
            Some(player),
        )
    }
}
