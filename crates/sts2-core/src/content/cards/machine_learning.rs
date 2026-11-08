//! `MachineLearning` (`Models.Cards/MachineLearning.cs`): Defect rare power, cost 1 (Innate
//! upgraded); gain 1 `MachineLearningPower`, which draws a card more each turn.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct MachineLearning;

#[sts2_macros::hooks]
impl Hooks for MachineLearning {
    type Me = CardId;
}

impl CardModel for MachineLearning {
    fn name(&self) -> &'static str {
        "Machine Learning"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1)];
        VARS
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Innate);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::MachineLearning,
            eng.s.card(play.card).var(Var::Cards),
            Some(player),
        )
    }
}
