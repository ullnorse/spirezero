//! `FanOfKnives` (`Models.Cards/FanOfKnives.cs`): cost 2, Power, Rare; applies
//! `FanOfKnivesPower` (Shivs hit every enemy), then creates 4 Shivs in the hand (5 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::shiv;
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct FanOfKnives;

#[sts2_macros::hooks]
impl Hooks for FanOfKnives {
    type Me = CardId;
}

impl CardModel for FanOfKnives {
    fn name(&self) -> &'static str {
        "Fan of Knives"
    }

    fn cost(&self) -> i32 {
        2
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Shivs"), 4).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let shivs = eng.s.card(play.card).int_var(Var::Named("Shivs"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::FanOfKnives,
            Decimal::ONE,
            Some(player),
        )?;
        for _ in 0..shivs {
            shiv::create_in_hand(eng, 1)?;
        }
        Ok(())
    }
}
