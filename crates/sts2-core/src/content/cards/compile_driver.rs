//! `CompileDriver` (`Models.Cards/CompileDriver.cs`): cost 1, 7 damage (10 upgraded), then draw one card per distinct orb kind in the queue.

use std::collections::HashSet;

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct CompileDriver;

#[sts2_macros::hooks]
impl Hooks for CompileDriver {
    type Me = CardId;
}

impl CardModel for CompileDriver {
    fn name(&self) -> &'static str {
        "Compile Driver"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 7).upgrade(3),
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::Named("CalculatedCards"), distinct_orb_kinds),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("CompileDriver needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let cards = eng
            .s
            .calculate(play.card, Var::Named("CalculatedCards"), play.target);
        eng.draw(cards.expect("CalculatedCards is declared"), false)?;
        Ok(())
    }
}

/// The distinct orb kinds in the owner's queue (`GroupBy(Id)`).
fn distinct_orb_kinds(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let kinds: HashSet<OrbKind> = s.player.orbs.iter().map(|o| o.kind).collect();
    Decimal::from(i32::try_from(kinds.len()).expect("at most 5 kinds"))
}
