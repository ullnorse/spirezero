//! `Supercritical` (`Models.Cards/Supercritical.cs`): Defect rare skill, cost 0, Exhaust; gain 4
//! energy (6 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct Supercritical;

#[sts2_macros::hooks]
impl Hooks for Supercritical {
    type Me = CardId;
}

impl CardModel for Supercritical {
    fn name(&self) -> &'static str {
        "Supercritical"
    }

    fn cost(&self) -> i32 {
        0
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 4).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.gain_energy(Decimal::from(eng.s.card(play.card).int_var(Var::Energy)))
    }
}
