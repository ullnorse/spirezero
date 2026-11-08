//! `Entropy` (`Models.Cards/Entropy.cs`): cost 1 power; gain 1 Entropy. Upgrading adds Innate.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Entropy;

#[sts2_macros::hooks]
impl Hooks for Entropy {
    type Me = CardId;
}

impl CardModel for Entropy {
    fn may_prompt(&self) -> bool {
        // The power it applies asks for a hand selection every turn.
        true
    }

    fn name(&self) -> &'static str {
        "Entropy"
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

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Innate);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let cards = Decimal::from(eng.s.card(play.card).int_var(Var::Cards));
        eng.apply_power(player, PowerKind::Entropy, cards, Some(player))
    }
}
