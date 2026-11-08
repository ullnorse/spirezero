//! `Spinner` (`Models.Cards/Spinner.cs`): power, cost 1; upgraded, channel 1 Glass first; then
//! apply `SpinnerPower` (1).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Spinner;

#[sts2_macros::hooks]
impl Hooks for Spinner {
    type Me = CardId;
}

impl CardModel for Spinner {
    fn name(&self) -> &'static str {
        "Spinner"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Spinner), 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng
            .s
            .card(play.card)
            .int_var(Var::Power(PowerKind::Spinner));
        let player = eng.s.player.creature;
        if eng.s.card(play.card).is_upgraded() {
            eng.channel(OrbKind::Glass)?;
        }
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Spinner,
            Decimal::from(amount),
            Some(player),
        )
    }
}
