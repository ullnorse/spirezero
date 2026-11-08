//! `GoldAxe` (`Models.Cards/GoldAxe.cs`): cost 1; damage equal to the number of cards played
//! this combat (`0 + 1 × CardPlaysFinished.Count()`); upgrading adds Retain.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::history::HistoryTag;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct GoldAxe;

#[sts2_macros::hooks]
impl Hooks for GoldAxe {
    type Me = CardId;
}

impl CardModel for GoldAxe {
    fn name(&self) -> &'static str {
        "Gold Axe"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Retain);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::ExtraDamage, 1),
            CardVar::calculated(Var::CalculatedDamage, card_plays_finished),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("GoldAxe needs a target");
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Calculated(play.card),
            Some(play.card),
        ))?;
        Ok(())
    }
}

/// `CombatManager.Instance.History.CardPlaysFinished.Count()`.
fn card_plays_finished(state: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(i64::from(state.history_count(HistoryTag::CardPlayFinished)))
}
