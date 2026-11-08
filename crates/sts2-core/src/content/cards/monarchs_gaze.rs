//! `MonarchsGaze` (`Models.Cards/MonarchsGaze.cs`): Regent rare power, cost 2 (1 upgraded); apply
//! `StrengthLoss` (1) `MonarchsGazePower`.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct MonarchsGaze;

#[sts2_macros::hooks]
impl Hooks for MonarchsGaze {
    type Me = CardId;
}

impl CardModel for MonarchsGaze {
    fn name(&self) -> &'static str {
        "Monarch's Gaze"
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

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("StrengthLoss"), 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::MonarchsGaze,
            eng.s.card(play.card).var(Var::Named("StrengthLoss")),
            Some(player),
        )
    }
}
