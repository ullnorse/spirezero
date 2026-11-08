//! `Unleash` (`Models.Cards/Unleash.cs`): Osty attacks for `6 + Osty's current HP`
//! (base 9 upgraded). Does nothing if Osty is missing or dead.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Unleash;

#[sts2_macros::hooks]
impl Hooks for Unleash {
    type Me = CardId;
}

impl CardModel for Unleash {
    fn name(&self) -> &'static str {
        "Unleash"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Basic
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 6).upgrade(3),
            CardVar::new(Var::ExtraDamage, 1),
            CardVar::calculated(Var::CalculatedDamage, osty_hp),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        on_play(eng, play.card, play.target)
    }
}

fn on_play(eng: &mut Engine<'_>, card: CardId, target: Option<CreatureId>) -> Flow {
    let target = target.expect("Unleash needs a target");
    // Osty.CheckMissingWithAnim.
    if !eng.s.is_osty_alive() {
        return Ok(());
    }
    let osty = eng.s.osty().expect("Osty is alive");
    // Dealer is Osty, so the player's Strength does not apply.
    eng.attack(&Attack::new(
        osty,
        AttackTargets::Single(target),
        AttackDamage::Calculated(card),
        Some(card),
    ))?;
    Ok(())
}

/// Osty's current HP if alive, else 0.
fn osty_hp(state: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    state
        .osty()
        .map(|o| state.creature(o))
        .filter(|o| o.is_alive())
        .map_or(Decimal::ZERO, |o| Decimal::from(o.hp))
}
