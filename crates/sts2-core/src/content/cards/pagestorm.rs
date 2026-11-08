//! `Pagestorm` (`Models.Cards/Pagestorm.cs`): cost 1 (0 upgraded) power (Ethereal is only a hover tip); drawing an
//! Ethereal card draws 1 more.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Pagestorm;

#[sts2_macros::hooks]
impl Hooks for Pagestorm {
    type Me = CardId;
}

impl CardModel for Pagestorm {
    fn name(&self) -> &'static str {
        "Pagestorm"
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

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let cards = eng.s.card(play.card).var(Var::Cards);
        eng.apply_power(player, PowerKind::Pagestorm, cards, Some(player))
    }
}
