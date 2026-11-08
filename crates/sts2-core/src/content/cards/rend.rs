//! `Rend` (`Models.Cards/Rend.cs`): cost 1; damage `10 + 5 ×` the number of the target's
//! non-temporary debuffs (`12 + 8 ×` upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerType;
use crate::decimal::Decimal;

pub struct Rend;

#[sts2_macros::hooks]
impl Hooks for Rend {
    type Me = CardId;
}

impl CardModel for Rend {
    fn name(&self) -> &'static str {
        "Rend"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 10).upgrade(2),
            CardVar::new(Var::ExtraDamage, 5).upgrade(3),
            CardVar::calculated(Var::CalculatedDamage, debuff_count),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Rend needs a target");
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

/// `target?.Powers.Count(ShouldCountPower) ?? 0`: debuffs for their current amount that are not
/// `ITemporaryPower`.
fn debuff_count(state: &CombatState, _card: CardId, target: Option<CreatureId>) -> Decimal {
    let count = target.map_or(0, |t| {
        state
            .creature(t)
            .powers
            .iter()
            .filter(|p| {
                p.kind.type_for_amount(Decimal::from(p.amount)) == PowerType::Debuff
                    && !p.kind.is_temporary()
            })
            .count()
    });
    Decimal::from(i32::try_from(count).expect("a power count fits an i32"))
}
