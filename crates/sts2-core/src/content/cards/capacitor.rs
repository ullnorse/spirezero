//! `Capacitor` (`Models.Cards/Capacitor.cs`): cost 1 power; 2 orb slots (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Capacitor;

#[sts2_macros::hooks]
impl Hooks for Capacitor {
    type Me = CardId;
}

impl CardModel for Capacitor {
    fn name(&self) -> &'static str {
        "Capacitor"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Repeat, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let slots = eng.s.card(play.card).int_var(Var::Repeat);
        eng.add_orb_slots(slots);
        Ok(())
    }
}
