//! `MementoMori` (`Models.Cards/MementoMori.cs`): cost 1, 9 damage (11 upgraded) plus 4 (5) per
//! card the owner discarded this turn.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, HistoryTag};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct MementoMori;

#[sts2_macros::hooks]
impl Hooks for MementoMori {
    type Me = CardId;
}

impl CardModel for MementoMori {
    fn name(&self) -> &'static str {
        "Memento Mori"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 9).upgrade(2),
            CardVar::new(Var::ExtraDamage, 4).upgrade(1),
            CardVar::calculated(Var::CalculatedDamage, discards_this_turn),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("MementoMori needs a target");
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

/// `CardDiscardedEntry`s this turn with the owner as actor.
fn discards_this_turn(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let player = s.player.creature;
    let count = s
        .history_this_turn(HistoryTag::CardDiscarded)
        .iter()
        .filter(|e| e.actor == player)
        .count();
    Decimal::from(i32::try_from(count).expect("a turn has few discards"))
}
