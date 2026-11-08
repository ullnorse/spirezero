//! `BadLuck` (`Models.Cards/BadLuck.cs`): curse, Eternal and unplayable. When the turn ends with
//! it in hand its owner loses 13 HP (unblockable, unpowered). `Reflections`' Shatter adds it.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct BadLuck;

#[sts2_macros::hooks]
impl Hooks for BadLuck {
    type Me = CardId;
}

impl CardModel for BadLuck {
    fn name(&self) -> &'static str {
        "Bad Luck"
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
        Keywords::NONE
            .with(CardKeyword::Eternal)
            .with(CardKeyword::Unplayable)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::HpLoss, 13)];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }

    fn has_turn_end_in_hand_effect(&self) -> bool {
        true
    }

    /// `CreatureCmd.Damage(Owner.Creature, HpLoss, Unblockable | Unpowered | Move, this, null)`.
    fn on_turn_end_in_hand(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        let owner = eng.s.player.creature;
        eng.damage(
            &[owner],
            eng.s.card(me).var(Var::HpLoss),
            ValueProps::UNBLOCKABLE
                .with(ValueProps::UNPOWERED)
                .with(ValueProps::MOVE),
            Some(owner),
            Some(me),
        )?;
        Ok(())
    }
}
