//! `Shadowmeld` (`Models.Cards/Shadowmeld.cs`): cost 1 (0 upgraded); Block gained this turn is doubled.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Shadowmeld;

#[sts2_macros::hooks]
impl Hooks for Shadowmeld {
    type Me = CardId;
}

impl CardModel for Shadowmeld {
    fn name(&self) -> &'static str {
        "Shadowmeld"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Power"), 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Shadowmeld,
            eng.s.card(play.card).var(Var::Named("Power")),
            Some(player),
        )
    }
}
