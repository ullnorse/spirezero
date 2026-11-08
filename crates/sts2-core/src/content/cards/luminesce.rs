//! `Luminesce` (`Models.Cards/Luminesce.cs`): token, cost 0, Exhaust and Retain; gain 2 energy
//! (3 upgraded). `RadiantPearl` creates it.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Luminesce;

#[sts2_macros::hooks]
impl Hooks for Luminesce {
    type Me = CardId;
}

impl CardModel for Luminesce {
    fn name(&self) -> &'static str {
        "Luminesce"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Token
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE
            .with(CardKeyword::Exhaust)
            .with(CardKeyword::Retain)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let energy = eng.s.card(play.card).var(Var::Energy);
        eng.gain_energy(energy)
    }
}
