//! `EvilEye` (`Models.Cards/EvilEye.cs`): gain 8 block (11 upgraded), twice if a card was exhausted
//! this turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, HistoryTag, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct EvilEye;

#[sts2_macros::hooks]
impl Hooks for EvilEye {
    type Me = CardId;
}

impl CardModel for EvilEye {
    fn name(&self) -> &'static str {
        "Evil Eye"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 8).upgrade(3)];
        VARS
    }

    /// `ShouldGlowGoldInternal`: `WasCardExhaustedThisTurn`.
    fn glows_gold(&self, s: &CombatState, _me: CardId) -> bool {
        was_card_exhausted_this_turn(s)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let exhausted = was_card_exhausted_this_turn(eng.s);
        let block = eng.s.card(play.card).var(Var::Block);
        for _ in 0..if exhausted { 2 } else { 1 } {
            eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        }
        Ok(())
    }
}

/// `WasCardExhaustedThisTurn`: a `CardExhaustedEntry` of the owner this turn.
fn was_card_exhausted_this_turn(s: &CombatState) -> bool {
    let player = s.player.creature;
    s.history_this_turn(HistoryTag::CardExhausted)
        .iter()
        .any(|e| e.actor == player)
}
