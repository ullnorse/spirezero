//! C23: `may_prompt` must hold for every model that can prompt, since release builds snapshot a
//! step only then (debug builds always snapshot, so `Combat::run` asserts it too).

use super::*;
use crate::content::potions::PotionKind;

#[test]
fn every_potion_that_prompts_reports_may_prompt() {
    // The 4 pt1 loadouts that panicked in release builds (T16) prompt from the draw pile,
    // discard pile and hand: the first turn has a draw pile, the second a discard pile.
    let mut prompted = Vec::new();
    for (kind, end_turn) in PotionKind::ALL
        .into_iter()
        .flat_map(|k| [(k, false), (k, true)])
    {
        let mut c = with_potions(&[kind]);
        if end_turn {
            c.apply(Action::EndTurn).expect("legal");
        }
        let Some(use_it) = c
            .legal_actions()
            .into_iter()
            .find(|a| matches!(a, Action::UsePotion { slot: 0, .. }))
        else {
            continue;
        };
        let status = c.apply(use_it).expect("legal");
        if status == Status::Decision {
            assert!(kind.may_prompt(), "{kind:?} prompted without `may_prompt`");
            prompted.push(kind);
        }
    }
    for kind in [
        PotionKind::DropletOfPrecognition,
        PotionKind::GamblersBrew,
        PotionKind::LiquidMemories,
        PotionKind::TouchOfInsanity,
    ] {
        assert!(prompted.contains(&kind), "{kind:?} did not prompt");
    }
}
