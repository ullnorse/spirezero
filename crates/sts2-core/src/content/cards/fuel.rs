//! `Fuel` (`Models.Cards/Fuel.cs`): token, cost 0, Exhaust; gain 1 energy (2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Fuel;

#[sts2_macros::hooks]
impl Hooks for Fuel {
    type Me = CardId;
}

impl CardModel for Fuel {
    fn name(&self) -> &'static str {
        "Fuel"
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
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let energy = eng.s.card(play.card).var(Var::Energy);
        eng.gain_energy(energy)
    }
}
