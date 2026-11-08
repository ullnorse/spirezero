//! `Apparition` (`Models.Cards/Apparition.cs`): Ancient skill (`DistinguishedCape`), cost 1,
//! Ethereal and Exhaust; gain 1 Intangible. The upgrade removes Ethereal.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Apparition;

#[sts2_macros::hooks]
impl Hooks for Apparition {
    type Me = CardId;
}

impl CardModel for Apparition {
    fn name(&self) -> &'static str {
        "Apparition"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE
            .with(CardKeyword::Ethereal)
            .with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Ethereal);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Intangible), 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Intangible,
            eng.s.card(play.card).var(Var::Power(PowerKind::Intangible)),
            Some(player),
        )
    }
}
