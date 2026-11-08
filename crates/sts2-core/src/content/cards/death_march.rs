//! `DeathMarch` (`Models.Cards/DeathMarch.cs`): cost 1, 8 damage (9 upgraded) plus 4 (6
//! upgraded) per card the player drew this turn outside the hand draw.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{
    CardId, CardPlay, CombatState, CreatureId, Engine, Flow, HistoryKind, HistoryTag,
};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct DeathMarch;

#[sts2_macros::hooks]
impl Hooks for DeathMarch {
    type Me = CardId;
}

impl CardModel for DeathMarch {
    fn name(&self) -> &'static str {
        "Death March"
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
            CardVar::new(Var::CalculationBase, 8).upgrade(1),
            CardVar::new(Var::ExtraDamage, 4).upgrade(2),
            CardVar::calculated(Var::CalculatedDamage, cards_drawn_outside_hand_draw),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("DeathMarch needs a target");
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

/// `CardDrawnEntry`s this turn with the owner as actor and `!FromHandDraw`.
fn cards_drawn_outside_hand_draw(
    s: &CombatState,
    _card: CardId,
    _target: Option<CreatureId>,
) -> Decimal {
    let me = s.player.creature;
    let count = s
        .history_this_turn(HistoryTag::CardDrawn)
        .iter()
        .filter(|e| {
            e.actor == me
                && matches!(
                    e.kind,
                    HistoryKind::CardDrawn {
                        from_hand_draw: false,
                        ..
                    }
                )
        })
        .count();
    Decimal::from(i32::try_from(count).expect("history size fits i32"))
}
