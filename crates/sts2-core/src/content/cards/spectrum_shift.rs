//! `SpectrumShift` (`Models.Cards/SpectrumShift.cs`): Regent uncommon power, cost 2 (1 upgraded);
//! apply `Cards` (1) `SpectrumShiftPower`.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct SpectrumShift;

#[sts2_macros::hooks]
impl Hooks for SpectrumShift {
    type Me = CardId;
}

impl CardModel for SpectrumShift {
    fn name(&self) -> &'static str {
        "Spectrum Shift"
    }

    fn cost(&self) -> i32 {
        2
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

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::SpectrumShift,
            eng.s.card(play.card).var(Var::Cards),
            Some(player),
        )
    }
}
