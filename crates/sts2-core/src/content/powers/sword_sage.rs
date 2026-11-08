//! `SwordSagePower` (`Models.Powers/SwordSagePower.cs`): a buff (`Counter`); every Sovereign
//! Blade of the owner replays `Amount` more times: the ones in combat when the power is applied
//! or stacked, and each non-clone one that enters later. Removing it takes `Amount` replays off
//! every blade.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::state::Power;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::CardKind;
use crate::content::powers::tangled::all_cards;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct SwordSage;

impl PowerModel for SwordSage {
    fn name(&self) -> &'static str {
        "Sword Sage"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn after_removed(&self, eng: &mut Engine<'_>, power: &Power, _old_owner: CreatureId) -> Flow {
        // `oldOwner.Player.PlayerCombatState.AllCards`: the owner is the only player.
        for card in all_cards(eng.s) {
            let card = eng.s.card_mut(card);
            if card.kind == CardKind::SovereignBlade {
                card.base_replay_count =
                    card.base_replay_count.saturating_add_signed(-power.amount);
            }
        }
        Ok(())
    }
}

/// `TryAddReplays`: the owner's blade gains `amount` replays.
fn try_add_replays(s: &mut CombatState, card: CardId, amount: i32) {
    let card = s.card_mut(card);
    if card.kind == CardKind::SovereignBlade {
        card.base_replay_count = card.base_replay_count.saturating_add_signed(amount);
    }
}

#[sts2_macros::hooks]
impl Hooks for SwordSage {
    type Me = PowerRef;

    fn after_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        power: PowerRef,
        amount: Decimal,
        _applier: Option<CreatureId>,
    ) -> Flow {
        if power != me {
            return Ok(());
        }
        let amount = amount.to_int_truncating();
        for card in all_cards(eng.s) {
            try_add_replays(eng.s, card, amount);
        }
        Ok(())
    }

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: PowerRef, card: CardId) -> Flow {
        if !eng.s.card(card).is_clone() {
            let amount = eng.s.power(me.owner, me.id).amount;
            try_add_replays(eng.s, card, amount);
        }
        Ok(())
    }
}
