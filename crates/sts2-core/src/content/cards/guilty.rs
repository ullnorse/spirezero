//! `Guilty` (`Models.Cards/Guilty.cs`): curse, unplayable (cost -1), no target. Its
//! `AfterCombatEnd` acts on the deck card (it leaves the deck after 5 combats), so the run layer
//! runs it (`run::cards`).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardData, CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

/// Curse added by `NeowsSacrifice`.
pub struct Guilty;

#[sts2_macros::hooks]
impl Hooks for Guilty {
    type Me = CardId;
}

impl CardModel for Guilty {
    fn name(&self) -> &'static str {
        "Guilty"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Combats"), 5)];
        VARS
    }

    /// The setter of `CombatsSeen`: the var is `5 - CombatsSeen`.
    fn instance_var(&self, card: &Card, name: Var, value: Decimal) -> Decimal {
        match (name, card.data) {
            (Var::Named("Combats"), CardData::Counter(seen)) => {
                value - Decimal::from(i32::try_from(seen).expect("counter fits i32"))
            }
            _ => value,
        }
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
