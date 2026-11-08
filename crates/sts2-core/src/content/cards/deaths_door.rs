//! `DeathsDoor` (`Models.Cards/DeathsDoor.cs`): cost 1; 6 Block (7 upgraded), repeated 2 more times if the player applied Doom this turn.

use crate::combat::ValueProps;
use crate::combat::history::{HistoryKind, HistoryTag};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct DeathsDoor;

#[sts2_macros::hooks]
impl Hooks for DeathsDoor {
    type Me = CardId;
}

impl CardModel for DeathsDoor {
    fn name(&self) -> &'static str {
        "Death's Door"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
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
            CardVar::new(Var::Block, 6).upgrade(1),
            CardVar::new(Var::Repeat, 2),
        ];
        VARS
    }

    /// `ShouldGlowGoldInternal`: `WasDoomAppliedThisTurn`.
    fn glows_gold(&self, s: &CombatState, _me: CardId) -> bool {
        was_doom_applied_this_turn(s)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        let block_gains = if was_doom_applied_this_turn(eng.s) {
            1 + eng.s.card(play.card).int_var(Var::Repeat)
        } else {
            1
        };
        for _ in 0..block_gains {
            eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        }
        Ok(())
    }
}

/// `WasDoomAppliedThisTurn`: a `PowerReceivedEntry` this turn for Doom applied by the owner.
fn was_doom_applied_this_turn(s: &CombatState) -> bool {
    let player = s.player.creature;
    s.history_this_turn(HistoryTag::PowerReceived)
        .iter()
        .any(|e| {
            matches!(&e.kind, HistoryKind::PowerReceived { kind: PowerKind::Doom, applier, .. }
                if *applier == Some(player))
        })
}
