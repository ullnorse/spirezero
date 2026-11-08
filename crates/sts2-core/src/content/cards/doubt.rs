//! `Doubt` (`Models.Cards/Doubt.cs`): curse, unplayable. When the turn ends with it in hand its owner
//! gets 1 Weak. A new debuff on the player skips its next duration tick (`PowerCmd.Apply` and the
//! card's own `SkipNextDurationTick` are the same rule), so it lasts the whole next turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Doubt;

#[sts2_macros::hooks]
impl Hooks for Doubt {
    type Me = CardId;
}

impl CardModel for Doubt {
    fn name(&self) -> &'static str {
        "Doubt"
    }

    fn cost(&self) -> i32 {
        -1
    }

    fn card_type(&self) -> CardType {
        CardType::Curse
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Curse
    }

    fn target(&self) -> TargetType {
        TargetType::None
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Unplayable)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Weak), 1)];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }

    fn has_turn_end_in_hand_effect(&self) -> bool {
        true
    }

    /// `PowerCmd.Apply<WeakPower>(Owner.Creature, PowerVar<WeakPower>(1m), null, this)`, then
    /// `SkipNextDurationTick = true` on a newly created power (already set by `add_power`).
    fn on_turn_end_in_hand(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        let owner = eng.s.player.creature;
        let weak = eng.s.card(me).var(Var::Power(PowerKind::Weak));
        eng.apply_power(owner, PowerKind::Weak, weak, None)
    }
}
