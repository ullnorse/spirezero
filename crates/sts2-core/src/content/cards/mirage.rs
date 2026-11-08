//! `Mirage` (`Models.Cards/Mirage.cs`): cost 1, Exhaust (removed by the upgrade); gains Block equal to the Poison on all living enemies.

use crate::combat::ValueProps;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Mirage;

#[sts2_macros::hooks]
impl Hooks for Mirage {
    type Me = CardId;
}

impl CardModel for Mirage {
    fn name(&self) -> &'static str {
        "Mirage"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
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

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::CalculatedBlock, living_enemy_poison),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng
            .s
            .calculate(play.card, Var::CalculatedBlock, play.target)
            .expect("Mirage has a CalculatedBlock var");
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}

/// The Poison of every living enemy.
fn living_enemy_poison(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let poison: i32 = s
        .enemies
        .iter()
        .map(|&e| s.creature(e))
        .filter(|e| e.is_alive())
        .map(|e| e.power_amount(PowerKind::Poison))
        .sum();
    Decimal::from(poison)
}
