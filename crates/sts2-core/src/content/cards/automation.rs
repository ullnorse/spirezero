//! `Automation` (`Models.Cards/Automation.cs`): power, cost 1 (0 upgraded); apply a new
//! `AutomationPower` instance of 1 energy.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Automation;

#[sts2_macros::hooks]
impl Hooks for Automation {
    type Me = CardId;
}

impl CardModel for Automation {
    fn name(&self) -> &'static str {
        "Automation"
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

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let amount = Decimal::from(eng.s.card(play.card).int_var(Var::Energy));
        eng.apply_power(player, PowerKind::Automation, amount, Some(player))
    }
}
