//! `Finisher` (`Models.Cards/Finisher.cs`): cost 1, 6 damage (8 upgraded) once per Attack the player finished playing this turn.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{
    CardId, CardPlay, CombatState, CreatureId, Engine, Flow, HistoryKind, HistoryTag,
};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Finisher;

#[sts2_macros::hooks]
impl Hooks for Finisher {
    type Me = CardId;
}

impl CardModel for Finisher {
    fn name(&self) -> &'static str {
        "Finisher"
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
            CardVar::new(Var::Damage, 6).upgrade(2),
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::Named("CalculatedHits"), attacks_finished_this_turn),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Finisher needs a target");
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

/// The owner's `CardPlayFinishedEntry`s this turn for Attacks (the card being played has no
/// entry yet).
fn attacks_finished_this_turn(
    s: &CombatState,
    _card: CardId,
    _target: Option<CreatureId>,
) -> Decimal {
    let player = s.player.creature;
    let count = s
        .history_this_turn(HistoryTag::CardPlayFinished)
        .iter()
        .filter(|e| {
            e.actor == player
                && matches!(
                    e.kind,
                    HistoryKind::CardPlayFinished {
                        card_type: CardType::Attack,
                        ..
                    }
                )
        })
        .count();
    Decimal::from(i32::try_from(count).expect("a turn has few card plays"))
}
