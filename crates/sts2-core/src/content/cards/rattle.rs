//! `Rattle` (`Models.Cards/Rattle.cs`): cost 1, Osty hits for 7 (9 upgraded), once plus once per
//! attack Osty made this turn. Does nothing without a living Osty.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, HistoryTag};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Rattle;

#[sts2_macros::hooks]
impl Hooks for Rattle {
    type Me = CardId;
}

impl CardModel for Rattle {
    fn name(&self) -> &'static str {
        "Rattle"
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

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::OstyDamage, 7).upgrade(2),
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::Named("CalculatedHits"), one_plus_osty_attacks),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Rattle needs a target");
        // Osty.CheckMissingWithAnim.
        if !eng.s.is_osty_alive() {
            return Ok(());
        }
        let damage = eng.s.card(play.card).var(Var::OstyDamage);
        let osty = eng.s.osty().expect("Osty is alive");
        let hits = eng
            .s
            .calculate_int(play.card, Var::Named("CalculatedHits"), play.target)
            .cast_unsigned();
        eng.attack(
            &Attack::new(
                osty,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}

/// 1 + the `CreatureAttackedEntry`s whose actor is Osty this turn.
fn one_plus_osty_attacks(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let osty = s.osty();
    let attacks = s
        .history_this_turn(HistoryTag::CreatureAttacked)
        .iter()
        .filter(|e| Some(e.actor) == osty)
        .count();
    Decimal::from(i32::try_from(1 + attacks).expect("a turn has few attacks"))
}
