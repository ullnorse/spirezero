//! `CreativeAi` (`Models.Cards/CreativeAi.cs`): power, cost 3; applies `CreativeAiPower`.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct CreativeAi;

#[sts2_macros::hooks]
impl Hooks for CreativeAi {
    type Me = CardId;
}

impl CardModel for CreativeAi {
    fn name(&self) -> &'static str {
        "Creative AI"
    }

    fn cost(&self) -> i32 {
        3
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

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("CreativeAi"), 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Named("CreativeAi"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::CreativeAi,
            amount,
            Some(player),
        )
    }
}
