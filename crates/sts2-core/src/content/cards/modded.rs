//! `Modded` (`Models.Cards/Modded.cs`): cost 0; add 1 orb slot, draw 1 card (2 upgraded), then the card costs 1 more for the combat.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Modded;

#[sts2_macros::hooks]
impl Hooks for Modded {
    type Me = CardId;
}

impl CardModel for Modded {
    fn name(&self) -> &'static str {
        "Modded"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Repeat, 1),
            CardVar::new(Var::Cards, 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let card = eng.s.card(play.card);
        let (slots, cards) = (card.int_var(Var::Repeat), card.var(Var::Cards));
        eng.add_orb_slots(slots);
        eng.draw(cards, false)?;
        eng.s.energy_cost_mut(play.card).add_this_combat(1);
        Ok(())
    }
}
