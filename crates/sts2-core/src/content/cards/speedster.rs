//! `Speedster` (`Models.Cards/Speedster.cs`): cost 2 power (Innate upgraded); deals 2 damage to
//! all enemies whenever a card is drawn outside the hand draw on the owner's turn.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Speedster;

#[sts2_macros::hooks]
impl Hooks for Speedster {
    type Me = CardId;
}

impl CardModel for Speedster {
    fn name(&self) -> &'static str {
        "Speedster"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Innate);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Speedster), 2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Speedster,
            Decimal::from(
                eng.s
                    .card(play.card)
                    .int_var(Var::Power(PowerKind::Speedster)),
            ),
            Some(player),
        )
    }
}
