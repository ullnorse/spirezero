//! `Turbo` (`Models.Cards/Turbo.cs`): Defect common skill, cost 0; gain 2 energy (3 upgraded), then a
//! `Void` into the discard pile.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Turbo;

#[sts2_macros::hooks]
impl Hooks for Turbo {
    type Me = CardId;
}

impl CardModel for Turbo {
    fn name(&self) -> &'static str {
        "Turbo"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.gain_energy(eng.s.card(play.card).var(Var::Energy))?;
        let card = eng.create_card(CardKind::Void);
        let player = eng.s.player.creature;
        eng.add_generated_card_to_combat(card, PileType::Discard, Some(player))?;
        Ok(())
    }
}
