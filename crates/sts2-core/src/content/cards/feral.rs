//! `Feral` (`Models.Cards/Feral.cs`): cost 2 (1 upgraded), Power, Uncommon; applies `FeralPower`
//! (1).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Feral;

#[sts2_macros::hooks]
impl Hooks for Feral {
    type Me = CardId;
}

impl CardModel for Feral {
    fn name(&self) -> &'static str {
        "Feral"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Feral), 1)];
        VARS
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // CreatureCmd.TriggerAnim is visual only.
        let amount = eng.s.card(play.card).int_var(Var::Power(PowerKind::Feral));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Feral,
            Decimal::from(amount),
            Some(player),
        )
    }
}
