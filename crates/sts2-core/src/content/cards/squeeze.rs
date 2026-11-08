//! `Squeeze` (`Models.Cards/Squeeze.cs`): cost 3, Osty attacks for `25 + 5 ×` (the other
//! `OstyAttack` cards in any combat pile); base 30 and 6 per card upgraded. Does nothing
//! without a living Osty.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::PileType;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Squeeze;

#[sts2_macros::hooks]
impl Hooks for Squeeze {
    type Me = CardId;
}

impl CardModel for Squeeze {
    fn name(&self) -> &'static str {
        "Squeeze"
    }

    fn cost(&self) -> i32 {
        3
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

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 25).upgrade(5),
            CardVar::new(Var::ExtraDamage, 5).upgrade(1),
            CardVar::calculated(Var::CalculatedDamage, osty_attack_cards),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Squeeze needs a target");
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

/// `PlayerCombatState.AllCards.Count(c => c.Tags.Contains(OstyAttack) && c != card)`.
fn osty_attack_cards(state: &CombatState, card: CardId, _target: Option<CreatureId>) -> Decimal {
    let count = PileType::LISTENER_ORDER
        .into_iter()
        .flat_map(|p| state.player.pile(p).iter().copied())
        .filter(|&c| c != card && state.card(c).kind.is_osty_attack())
        .count();
    Decimal::from(i32::try_from(count).expect("a combat has few cards"))
}
