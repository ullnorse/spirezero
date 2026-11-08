//! `Shame` (`Models.Cards/Shame.cs`): curse, unplayable. When the turn ends with it in hand its owner
//! gets 1 Frail. A new debuff on the player skips its next duration tick (`PowerCmd.Apply` and the
//! card's own `SkipNextDurationTick` are the same rule), so it lasts the whole next turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Shame;

#[sts2_macros::hooks]
impl Hooks for Shame {
    type Me = CardId;
}

impl CardModel for Shame {
    fn name(&self) -> &'static str {
        "Shame"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Frail"), 1)];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }

    fn has_turn_end_in_hand_effect(&self) -> bool {
        true
    }

    /// `PowerCmd.Apply<FrailPower>(Owner.Creature, DynamicVar("Frail", 1m), null, this)`, then
    /// `SkipNextDurationTick = true` on a newly created power (already set by `add_power`).
    fn on_turn_end_in_hand(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        let owner = eng.s.player.creature;
        let frail = eng.s.card(me).var(Var::Named("Frail"));
        eng.apply_power(owner, PowerKind::Frail, frail, None)
    }
}
