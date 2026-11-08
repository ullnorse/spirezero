//! `Infection` (`Models.Cards/Infection.cs`): status, unplayable (cost -1), no target. When the
//! turn ends with it in hand it deals 3 unpowered damage to its owner.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

/// Status card added by `PhrogParasite` and `Wriggler`.
pub struct Infection;

#[sts2_macros::hooks]
impl Hooks for Infection {
    type Me = CardId;
}

impl CardModel for Infection {
    fn name(&self) -> &'static str {
        "Infection"
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
        Keywords::NONE.with(CardKeyword::Unplayable)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] =
            &[CardVar::new(Var::Damage, 3).props(ValueProps::UNPOWERED.with(ValueProps::MOVE))];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }

    fn has_turn_end_in_hand_effect(&self) -> bool {
        true
    }

    /// `CreatureCmd.Damage(Owner.Creature, DynamicVars.Damage, this, null)`: the dealer is the
    /// card's owner (`cardSource?.Owner.Creature`).
    fn on_turn_end_in_hand(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        let owner = eng.s.player.creature;
        eng.damage(
            &[owner],
            eng.s.card(me).var(Var::Damage),
            ValueProps::UNPOWERED.with(ValueProps::MOVE),
            Some(owner),
            Some(me),
        )?;
        Ok(())
    }
}
