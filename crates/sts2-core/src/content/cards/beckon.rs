//! `Beckon` (`Models.Cards/Beckon.cs`): status card (cost 1, playable, no effect). When the turn ends
//! with it in hand its owner loses 6 HP, unblockable and unpowered.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var};

/// Status card added by `SoulFysh`.
pub struct Beckon;

#[sts2_macros::hooks]
impl Hooks for Beckon {
    type Me = CardId;
}

impl CardModel for Beckon {
    fn name(&self) -> &'static str {
        "Beckon"
    }

    fn cost(&self) -> i32 {
        1
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
        Keywords::NONE
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::HpLoss, 6)];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }

    fn has_turn_end_in_hand_effect(&self) -> bool {
        true
    }

    /// `CreatureCmd.Damage(Owner.Creature, DynamicVars.HpLoss.BaseValue, Unblockable | Unpowered |
    /// Move, this, null)`: no dealer.
    fn on_turn_end_in_hand(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        let owner = eng.s.player.creature;
        eng.damage(
            &[owner],
            eng.s.card(me).var(Var::HpLoss),
            ValueProps::UNBLOCKABLE
                .with(ValueProps::UNPOWERED)
                .with(ValueProps::MOVE),
            None,
            Some(me),
        )?;
        Ok(())
    }
}
