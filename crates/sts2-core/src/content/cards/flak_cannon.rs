//! `FlakCannon` (`Models.Cards/FlakCannon.cs`): cost 2, 8 damage (11 upgraded) at a random enemy once per Status card
//! of the player's combat piles that is not in the Exhaust pile; those Statuses are exhausted first.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, PileType};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct FlakCannon;

#[sts2_macros::hooks]
impl Hooks for FlakCannon {
    type Me = CardId;
}

impl CardModel for FlakCannon {
    fn name(&self) -> &'static str {
        "Flak Cannon"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::RandomEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 8).upgrade(3),
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::Named("CalculatedHits"), status_count),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let statuses = statuses(eng.s);
        let hits = eng
            .s
            .calculate_int(play.card, Var::Named("CalculatedHits"), play.target)
            .cast_unsigned();
        for status in statuses {
            eng.exhaust(status, false)?;
        }
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::RandomOpponents,
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}

/// `GetStatuses`: the Status cards of `PlayerCombatState.AllCards` outside the Exhaust pile.
fn statuses(s: &CombatState) -> Vec<CardId> {
    [
        PileType::Hand,
        PileType::Draw,
        PileType::Discard,
        PileType::Play,
    ]
    .into_iter()
    .flat_map(|pile| s.player.pile(pile).iter().copied())
    .filter(|&c| s.card(c).card_type() == CardType::Status)
    .collect()
}

fn status_count(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(i32::try_from(statuses(s).len()).expect("the piles are small"))
}
