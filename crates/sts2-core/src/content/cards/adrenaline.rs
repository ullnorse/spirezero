//! `Adrenaline` (`Models.Cards/Adrenaline.cs`): cost 0, Exhaust, gain 1 energy (2 upgraded), draw 2.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Adrenaline;

#[sts2_macros::hooks]
impl Hooks for Adrenaline {
    type Me = CardId;
}

impl CardModel for Adrenaline {
    fn name(&self) -> &'static str {
        "Adrenaline"
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Energy, 1).upgrade(1),
            CardVar::new(Var::Cards, 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.gain_energy(eng.s.card(play.card).var(Var::Energy))?;
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        Ok(())
    }
}
