//! `SentryMode` (`Models.Cards/SentryMode.cs`): cost 2 (1 upgraded) power; before each hand
//! draw, a Sweeping Gaze joins the hand.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct SentryMode;

#[sts2_macros::hooks]
impl Hooks for SentryMode {
    type Me = CardId;
}

impl CardModel for SentryMode {
    fn name(&self) -> &'static str {
        "Sentry Mode"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::SentryMode), 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::SentryMode));
        eng.apply_power(player, PowerKind::SentryMode, amount, Some(player))
    }
}
