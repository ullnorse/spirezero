//! `ChildOfTheStars` (`Models.Cards/ChildOfTheStars.cs`): Regent uncommon power, cost 1; gain 2 Child of the Stars (3 upgraded): block per star spent.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct ChildOfTheStars;

#[sts2_macros::hooks]
impl Hooks for ChildOfTheStars {
    type Me = CardId;
}

impl CardModel for ChildOfTheStars {
    fn name(&self) -> &'static str {
        "Child of the Stars"
    }

    fn cost(&self) -> i32 {
        1
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("BlockForStars"), 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Named("BlockForStars"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::ChildOfTheStars,
            amount,
            Some(player),
        )
    }
}
