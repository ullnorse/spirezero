//! `WasteAway` (`Models.Cards/WasteAway.cs`): status, cost -1, no target, never upgraded. One of the
//! `KnowledgeDemon`'s Curse of Knowledge choices (`KnowledgeDemon.IChoosable`): it never enters a
//! pile; when chosen, `OnChosen` applies 1 `WasteAwayPower` to its owner. Playing it does
//! nothing.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// A Curse of Knowledge choice.
pub struct WasteAway;

#[sts2_macros::hooks]
impl Hooks for WasteAway {
    type Me = CardId;
}

impl CardModel for WasteAway {
    fn name(&self) -> &'static str {
        "Waste Away"
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

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::WasteAway), 1)];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}

/// `OnChosen`: `PowerCmd.Apply<WasteAwayPower>(Owner.Creature, DynamicVars["WasteAwayPower"].IntValue,
/// Owner.Creature, this)`.
pub fn on_chosen(eng: &mut Engine<'_>, me: CardId) -> Flow {
    let owner = eng.s.player.creature;
    let amount = Decimal::from(eng.s.card(me).int_var(Var::Power(PowerKind::WasteAway)));
    eng.apply_power_from_card(Some(me), owner, PowerKind::WasteAway, amount, Some(owner))
}
