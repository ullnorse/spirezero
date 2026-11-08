//! `Arsenal` (`Models.Cards/Arsenal.cs`): Regent rare power, cost 1; apply 1 `ArsenalPower`. Upgrading adds Innate.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Arsenal;

#[sts2_macros::hooks]
impl Hooks for Arsenal {
    type Me = CardId;
}

impl CardModel for Arsenal {
    fn name(&self) -> &'static str {
        "Arsenal"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Arsenal), 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Arsenal,
            eng.s.card(play.card).var(Var::Power(PowerKind::Arsenal)),
            Some(player),
        )
    }
}
