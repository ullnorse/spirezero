//! `TearAsunder` (`Models.Cards/TearAsunder.cs`): cost 2, 5 damage (7 upgraded), hitting once plus once per time you lost HP this combat.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct TearAsunder;

#[sts2_macros::hooks]
impl Hooks for TearAsunder {
    type Me = CardId;
}

impl CardModel for TearAsunder {
    fn name(&self) -> &'static str {
        "Tear Asunder"
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
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 5).upgrade(2),
            CardVar::new(Var::Repeat, 1),
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::Named("CalculatedHits"), one_plus_hp_loss_hits),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("TearAsunder needs a target");
        let player = eng.s.player.creature;
        let hits = eng
            .s
            .calculate_int(play.card, Var::Named("CalculatedHits"), play.target)
            .cast_unsigned();
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(eng.s.card(play.card).var(Var::Damage)),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}

/// 1 + the `DamageReceivedEntry`s with the owner as receiver and `UnblockedDamage > 0` this
/// combat.
fn one_plus_hp_loss_hits(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(i64::from(1 + s.hp_loss_hits_this_combat(s.player.creature)))
}
