//! What must hold at every position a fight stops at, whatever was played: the checks the FFI
//! fuzz target (`fuzz/fuzz_targets/ffi_step.rs`) runs after every step.

use sts2_core::Combat;
use sts2_core::combat::PileType;

use crate::action::{ACTIONS, HAND_SLOTS, MASK_WORDS};
use crate::run_action::{RUN_ACTIONS, RUN_MASK_WORDS};

/// Checks the invariants of `combat`'s state.
///
/// HP within `0..=max HP` and block non-negative for every creature, energy and stars
/// non-negative, at most [`HAND_SLOTS`] cards in hand, and pile conservation (every pile entry
/// is a card of the combat, no card is in two piles or twice in one, and each card's recorded
/// pile is the one that holds it).
///
/// # Errors
/// The first violation.
pub fn check_invariants(combat: &Combat) -> Result<(), String> {
    let s = combat.state();
    for c in &s.creatures {
        if c.max_hp < 0 || c.hp < 0 || c.hp > c.max_hp {
            return Err(format!(
                "creature {:?}: hp {} of max {}",
                c.id, c.hp, c.max_hp
            ));
        }
        if c.block < 0 {
            return Err(format!("creature {:?}: block {}", c.id, c.block));
        }
    }
    let p = &s.player;
    if p.energy < 0 || p.stars < 0 {
        return Err(format!("energy {} stars {}", p.energy, p.stars));
    }
    if p.hand.len() > HAND_SLOTS {
        return Err(format!("{} cards in hand", p.hand.len()));
    }
    let mut holder: Vec<Option<PileType>> = vec![None; s.cards.len()];
    for pile in PileType::LISTENER_ORDER {
        for &card in p.pile(pile) {
            let Some(slot) = holder.get_mut(card.0 as usize) else {
                return Err(format!("{pile:?} holds {card:?}, not a card of the combat"));
            };
            if let Some(other) = slot.replace(pile) {
                return Err(format!("{card:?} is in {other:?} and in {pile:?}"));
            }
        }
    }
    for (card, &pile) in s.cards.iter().zip(&holder) {
        if s.pile_of(card.id) != pile {
            return Err(format!(
                "{:?} is recorded in {:?} but held by {pile:?}",
                card.id,
                s.pile_of(card.id)
            ));
        }
    }
    Ok(())
}

/// Checks a mask buffer: no bit at or past [`ACTIONS`].
///
/// # Errors
/// If one is set.
pub fn check_mask_bits(mask: &[u64; MASK_WORDS]) -> Result<(), String> {
    let last = ACTIONS % 64;
    if last != 0 && mask[MASK_WORDS - 1] >> last != 0 {
        return Err(format!("mask bits past {ACTIONS}: {mask:x?}"));
    }
    Ok(())
}

/// Checks a run mask buffer: no bit at or past [`RUN_ACTIONS`].
///
/// # Errors
/// If one is set.
pub fn check_run_mask_bits(mask: &[u64; RUN_MASK_WORDS]) -> Result<(), String> {
    let last = RUN_ACTIONS % 64;
    if last != 0 && mask[RUN_MASK_WORDS - 1] >> last != 0 {
        return Err(format!("mask bits past {RUN_ACTIONS}: {mask:x?}"));
    }
    Ok(())
}
