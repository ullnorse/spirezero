//! `Disintegration` (`Models.Cards/Disintegration.cs`): status, cost -1, no target, never upgraded. One of the
//! `KnowledgeDemon`'s Curse of Knowledge choices (`KnowledgeDemon.IChoosable`): it never enters a
//! pile; when chosen, `OnChosen` applies its `BaseValue` of `DisintegrationPower` to its owner. Playing it does
//! nothing.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// A Curse of Knowledge choice.
pub struct Disintegration;

#[sts2_macros::hooks]
impl Hooks for Disintegration {
    type Me = CardId;
}

impl CardModel for Disintegration {
    fn name(&self) -> &'static str {
        "Disintegration"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Disintegration), 6)];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}

/// `OnChosen`: `PowerCmd.Apply<DisintegrationPower>(Owner.Creature,
/// DynamicVars["DisintegrationPower"].BaseValue, Owner.Creature, this)`. The `KnowledgeDemon` sets
/// that `BaseValue` on the created card (6, 7 or 8 by the curse's index) and passes it as `amount`.
pub fn on_chosen(eng: &mut Engine<'_>, me: CardId, amount: Decimal) -> Flow {
    let owner = eng.s.player.creature;
    eng.apply_power_from_card(
        Some(me),
        owner,
        PowerKind::Disintegration,
        amount,
        Some(owner),
    )
}
