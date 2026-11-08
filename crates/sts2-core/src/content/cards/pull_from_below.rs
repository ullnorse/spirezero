//! `PullFromBelow` (`Models.Cards/PullFromBelow.cs`): cost 1, 5 damage (7 upgraded) once per
//! Ethereal card the player played this combat.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct PullFromBelow;

#[sts2_macros::hooks]
impl Hooks for PullFromBelow {
    type Me = CardId;
}

impl CardModel for PullFromBelow {
    fn name(&self) -> &'static str {
        "Pull from Below"
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
            CardVar::calculated(Var::Named("CalculatedHits"), ethereal_plays_finished),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("PullFromBelow needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let hits = eng
            .s
            .calculate_int(play.card, Var::Named("CalculatedHits"), play.target)
            .cast_unsigned();
        let player = eng.s.player.creature;
        let attack = Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        )
        .with_hits(hits);
        eng.attack(&attack)?;
        Ok(())
    }
}

/// The owner's finished plays that were Ethereal.
fn ethereal_plays_finished(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(i64::from(s.ethereal_card_plays_finished()))
}
