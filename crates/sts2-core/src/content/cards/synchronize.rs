//! `Synchronize` (`Models.Cards/Synchronize.cs`): cost 1 Skill; Focus until the end of the turn
//! (`SynchronizePower`) = 0 + 1 (2 upgraded) per distinct orb kind in the queue, read at play time
//! (`GroupBy(orb.Id).Count()`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Synchronize;

/// `Var::Named("CalculatedFocus")`.
const CALCULATED_FOCUS: Var = Var::Named("CalculatedFocus");

#[sts2_macros::hooks]
impl Hooks for Synchronize {
    type Me = CardId;
}

impl CardModel for Synchronize {
    fn name(&self) -> &'static str {
        "Synchronize"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1).upgrade(1),
            CardVar::calculated(CALCULATED_FOCUS, distinct_orbs),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let focus = eng
            .s
            .calculate(play.card, CALCULATED_FOCUS, play.target)
            .expect("Synchronize has CalculatedFocus");
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Synchronize,
            focus,
            Some(player),
        )
    }
}

/// The number of distinct orb ids in the owner's queue.
fn distinct_orbs(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let kinds = s
        .player
        .orbs
        .iter()
        .fold(0_u8, |seen, orb| seen | 1 << orb.kind.index());
    Decimal::from(kinds.count_ones().cast_signed())
}
