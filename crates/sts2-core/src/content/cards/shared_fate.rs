//! `SharedFate` (`Models.Cards/SharedFate.cs`): cost 0, Exhaust; the player loses 2 Strength and
//! the target loses 2 (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct SharedFate;

#[sts2_macros::hooks]
impl Hooks for SharedFate {
    type Me = CardId;
}

impl CardModel for SharedFate {
    fn name(&self) -> &'static str {
        "Shared Fate"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Named("EnemyStrengthLoss"), 2).upgrade(1),
            CardVar::new(Var::Named("PlayerStrengthLoss"), 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("SharedFate needs a target");
        let player_loss = eng.s.card(play.card).var(Var::Named("PlayerStrengthLoss"));
        let enemy_loss = eng.s.card(play.card).var(Var::Named("EnemyStrengthLoss"));
        let player = eng.s.player.creature;
        eng.apply_power(
            player,
            PowerKind::Strength,
            Decimal::ZERO - player_loss,
            Some(player),
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Strength,
            Decimal::ZERO - enemy_loss,
            Some(player),
        )
    }
}
