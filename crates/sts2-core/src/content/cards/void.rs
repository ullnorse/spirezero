//! `Void` (`Models.Cards/Void.cs`): status, unplayable, Ethereal. Drawing it loses 1 energy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Void;

#[sts2_macros::hooks]
impl Hooks for Void {
    type Me = CardId;

    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: CardId,
        card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        if card == me {
            // `PlayerCmd.LoseEnergy(DynamicVars.Energy.IntValue, Owner)`.
            let energy = eng.s.card(me).var(Var::Energy);
            eng.lose_energy(energy);
        }
        Ok(())
    }
}

impl CardModel for Void {
    fn name(&self) -> &'static str {
        "Void"
    }

    fn cost(&self) -> i32 {
        -1
    }

    fn card_type(&self) -> CardType {
        CardType::Status
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Status
    }

    fn target(&self) -> TargetType {
        TargetType::None
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE
            .with(CardKeyword::Unplayable)
            .with(CardKeyword::Ethereal)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 1)];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
