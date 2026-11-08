//! `Protector` (`Models.Cards/Protector.cs`): cost 1 (0 upgraded), Osty attacks for `10 + 1 ×
//! Osty.MaxHp` (base 15 upgraded). Does nothing without a living Osty.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Protector;

#[sts2_macros::hooks]
impl Hooks for Protector {
    type Me = CardId;
}

impl CardModel for Protector {
    fn name(&self) -> &'static str {
        "Protector"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 10).upgrade(5),
            CardVar::new(Var::ExtraDamage, 1),
            CardVar::calculated(Var::CalculatedDamage, osty_max_hp).from_osty(),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Protector needs a target");
        // Osty.CheckMissingWithAnim.
        if !eng.s.is_osty_alive() {
            return Ok(());
        }
        let osty = eng.s.osty().expect("Osty is alive");
        eng.attack(&Attack::new(
            osty,
            AttackTargets::Single(target),
            AttackDamage::Calculated(play.card),
            Some(play.card),
        ))?;
        Ok(())
    }
}

/// Osty's max HP if alive, else 0.
fn osty_max_hp(state: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    state
        .osty()
        .map(|o| state.creature(o))
        .filter(|o| o.is_alive())
        .map_or(Decimal::ZERO, |o| Decimal::from(o.max_hp))
}
