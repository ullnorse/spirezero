//! `JackOfAllTrades` (`Models.Cards/JackOfAllTrades.cs`): cost 0, Exhaust; adds 1 (2 upgraded)
//! random Colorless cards other than itself to the hand (`GetDistinctForCombat` on
//! `Rng.CombatCardGeneration`).
//!
//! Generation stops the combat (`Unsupported`) if it picks a Colorless card that is not ported.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var, pools,
};

pub struct JackOfAllTrades;

#[sts2_macros::hooks]
impl Hooks for JackOfAllTrades {
    type Me = CardId;
}

impl CardModel for JackOfAllTrades {
    fn name(&self) -> &'static str {
        "Jack of All Trades"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let count = eng.s.card(play.card).count_var_len(Var::Cards);
        let pool: Vec<_> = pools::colorless()
            .into_iter()
            .filter(|c| c.id != "JACK_OF_ALL_TRADES")
            .collect();
        let cards = eng.get_distinct_for_combat(&pool, count)?;
        let player = eng.s.player.creature;
        for card in cards {
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
