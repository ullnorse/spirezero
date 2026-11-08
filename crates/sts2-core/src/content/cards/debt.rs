//! `Debt` (`Models.Cards/Debt.cs`): curse, unplayable. When the turn ends with it in hand the
//! player loses 10 gold (at most what they have).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct Debt;

#[sts2_macros::hooks]
impl Hooks for Debt {
    type Me = CardId;
}

impl CardModel for Debt {
    fn name(&self) -> &'static str {
        "Debt"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Gold, 10)];
        VARS
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }

    fn has_turn_end_in_hand_effect(&self) -> bool {
        true
    }

    /// `PlayerCmd.LoseGold(Math.Min(Gold, Owner.Gold), Owner)`.
    fn on_turn_end_in_hand(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        let lost = eng.s.card(me).int_var(Var::Gold).min(eng.s.player.gold);
        eng.lose_gold(Decimal::from(lost));
        Ok(())
    }
}
