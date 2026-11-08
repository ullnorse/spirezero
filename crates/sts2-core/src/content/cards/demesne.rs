//! `Demesne` (`Models.Cards/Demesne.cs`): cost 3 (2 upgraded), Ethereal; +1 card drawn and +1 max energy each turn.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Demesne;

#[sts2_macros::hooks]
impl Hooks for Demesne {
    type Me = CardId;
}

impl CardModel for Demesne {
    fn name(&self) -> &'static str {
        "Demesne"
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Ethereal)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 1), CardVar::new(Var::Cards, 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let cards = eng.s.card(play.card).var(Var::Cards);
        eng.apply_power(player, PowerKind::Demesne, cards, Some(player))
    }
}
