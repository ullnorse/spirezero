//! `HelixDrill` (`Models.Cards/HelixDrill.cs`): cost 0, 3 damage (5 upgraded) once per energy spent this turn, not counting the cost
//! of this card while it is being played.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{
    CardId, CardPlay, CombatState, CreatureId, Engine, Flow, HistoryKind, HistoryTag, PileType,
};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct HelixDrill;

#[sts2_macros::hooks]
impl Hooks for HelixDrill {
    type Me = CardId;
}

impl CardModel for HelixDrill {
    fn name(&self) -> &'static str {
        "Helix Drill"
    }

    fn cost(&self) -> i32 {
        0
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 3).upgrade(2),
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::Named("CalculatedHits"), energy_spent_this_turn),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("HelixDrill needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let hits = eng
            .s
            .calculate_int(play.card, Var::Named("CalculatedHits"), play.target)
            .cast_unsigned();
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}

/// The `CalculatedHits` multiplier: the `EnergySpentEntry` amounts of this turn, minus the card's
/// own cost while it is in the Play pile; 0 for a card in no pile.
fn energy_spent_this_turn(s: &CombatState, card: CardId, _target: Option<CreatureId>) -> Decimal {
    let Some(pile) = s.card(card).pile else {
        return Decimal::ZERO;
    };
    let mut spent: i32 = s
        .history_this_turn(HistoryTag::EnergySpent)
        .iter()
        .map(|e| match e.kind {
            HistoryKind::EnergySpent(amount) => amount,
            _ => 0,
        })
        .sum();
    if pile == PileType::Play {
        spent -= s.energy_cost(card);
    }
    Decimal::from(spent)
}
