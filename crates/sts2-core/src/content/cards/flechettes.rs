//! `Flechettes` (`Models.Cards/Flechettes.cs`): cost 1, 5 damage (7 upgraded) once per Skill in the hand.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Flechettes;

#[sts2_macros::hooks]
impl Hooks for Flechettes {
    type Me = CardId;
}

impl CardModel for Flechettes {
    fn name(&self) -> &'static str {
        "Flechettes"
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
            CardVar::new(Var::Damage, 5).upgrade(2),
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::Named("CalculatedHits"), skills_in_hand),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Flechettes needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        let hits = eng
            .s
            .calculate_int(play.card, Var::Named("CalculatedHits"), play.target)
            .cast_unsigned();
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

/// The Skills in the owner's hand.
fn skills_in_hand(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let count = s
        .player
        .hand
        .iter()
        .filter(|&&c| s.card(c).card_type() == CardType::Skill)
        .count();
    Decimal::from(i32::try_from(count).expect("the hand is small"))
}
