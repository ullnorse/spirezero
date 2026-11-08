//! `Radiate` (`Models.Cards/Radiate.cs`): Regent uncommon attack, cost 0, all enemies; deal 3
//! damage (4 upgraded) once per star the owner gained this turn (the positive `StarsModifiedEntry`
//! amounts).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{
    CardId, CardPlay, CombatState, CreatureId, Engine, Flow, HistoryKind, HistoryTag,
};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Radiate;

#[sts2_macros::hooks]
impl Hooks for Radiate {
    type Me = CardId;
}

impl CardModel for Radiate {
    fn name(&self) -> &'static str {
        "Radiate"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 3).upgrade(1),
            CardVar::new(Var::Stars, 1),
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::Named("CalculatedHits"), stars_gained_this_turn),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        let hits = eng
            .s
            .calculate_int(play.card, Var::Named("CalculatedHits"), play.target)
            .cast_unsigned();
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::AllOpponents,
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}

/// The sum of the owner's positive `StarsModifiedEntry` amounts this turn.
fn stars_gained_this_turn(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let player = s.player.creature;
    let total: i32 = s
        .history_this_turn(HistoryTag::StarsModified)
        .iter()
        .filter(|e| e.actor == player)
        .filter_map(|e| match e.kind {
            HistoryKind::StarsModified(amount) if amount > 0 => Some(amount),
            _ => None,
        })
        .sum();
    Decimal::from(total)
}
