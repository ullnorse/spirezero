//! The canonical digest of a combat state: one `u64` that every layout of [`CombatState`] must
//! reproduce.
//!
//! It serves the lockstep oracle (`crates/sts2-lockstep`) and the RL bench's digest
//! (`docs/specs/rl-2x-plan.md`, stage 0).
//!
//! "Canonical" means it hashes what the game state *is*, never how it is stored: creatures in id
//! order with their powers in application order (each with its instance data), removed powers,
//! the five piles in listener order (top first), every combat card in id order with its cost
//! lists, the player, the run's RNG streams (counter and state words), the history and the
//! combat's own fields. It leaves out caches that are derived from the rest (a creature's
//! `power_kinds`, the living-primary-enemy count, a card's `pile`, the hook unions) and the
//! next-id counters (every id they hand out is hashed where it is used), so a stage that drops or
//! re-lays out one of those keeps the digest.
//!
//! The structure is written field by field here; leaf values (a monster's move machine and RNG, a
//! card's keywords and costs, content data enums, history entries) go through their derived
//! `Hash` into [`Fnv`]. A stage that changes how such a leaf hashes changes the digest, and the
//! lockstep harness then reports it at the first step: keep the words the same (a manual `Hash`)
//! or bump the harness's pinned engine in the same change.
//!
//! [`canonical_parts`] gives the same words split into labelled parts, so a harness that finds
//! two digests unequal can name the first part that differs.

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::combat::{CombatState, PileType, state::Power};
use crate::rng::RunRngType;

/// FNV-1a over bytes, as a [`Hasher`] (fixed keys, so the same state hashes the same in every
/// process and build).
#[derive(Clone, Copy, Debug)]
pub struct Fnv(u64);

impl Fnv {
    const OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01B3;

    /// An empty hash.
    #[must_use]
    pub const fn new() -> Self {
        Self(Self::OFFSET)
    }
}

impl Default for Fnv {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher for Fnv {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(Self::PRIME);
        }
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

/// A labelled part of the digest ([`canonical_parts`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    /// The combat's own fields: encounter, round, side, result, enemy and ally lists, the deck.
    Combat,
    /// The run's RNG streams.
    Rng,
    /// The player's fields other than the piles.
    Player,
    /// A creature, its powers and removed powers, its monster.
    Creature(u32),
    /// A pile's cards, in order.
    Pile(PileType),
    /// A combat card and its cost lists.
    Card(u32),
    /// The combat history.
    History,
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Combat => f.write_str("combat"),
            Self::Rng => f.write_str("rng"),
            Self::Player => f.write_str("player"),
            Self::Creature(id) => write!(f, "creature {id}"),
            Self::Pile(pile) => write!(f, "pile {pile:?}"),
            Self::Card(id) => write!(f, "card {id}"),
            Self::History => f.write_str("history"),
        }
    }
}

/// The canonical digest of `s` (module docs).
#[must_use]
pub fn canonical_digest(s: &CombatState) -> u64 {
    let mut total = Fnv::new();
    visit(s, &mut |_, part| total.write_u64(part));
    total.finish()
}

/// The parts [`canonical_digest`] folds, in order, each with its own hash.
#[must_use]
pub fn canonical_parts(s: &CombatState) -> Vec<(Part, u64)> {
    let mut parts = Vec::new();
    visit(s, &mut |label, part| parts.push((label, part)));
    parts
}

/// One power instance and its data.
fn power(s: &CombatState, p: &Power, h: &mut Fnv) {
    p.id.0.hash(h);
    p.kind.hash(h);
    p.amount.hash(h);
    p.amount_on_turn_start.hash(h);
    p.applier.map(|c| c.0).hash(h);
    p.skip_next_duration_tick.hash(h);
    s.power_data(p.id).hash(h);
}

/// Hashes each part of `s` and hands it to `sink`.
fn visit(s: &CombatState, sink: &mut impl FnMut(Part, u64)) {
    let mut h = Fnv::new();
    s.ascension.hash(&mut h);
    s.encounter.hash(&mut h);
    s.deck.hash(&mut h);
    s.room_type.hash(&mut h);
    s.act_index.hash(&mut h);
    s.map_coord.hash(&mut h);
    s.round.hash(&mut h);
    s.side.hash(&mut h);
    ids(&mut h, s.allies.iter().map(|c| c.0));
    ids(&mut h, s.enemies.iter().map(|c| c.0));
    s.in_progress.hash(&mut h);
    s.starting.hash(&mut h);
    s.pending_loss.hash(&mut h);
    s.result.hash(&mut h);
    s.extra_rewards.hash(&mut h);
    s.removed_from_deck.hash(&mut h);
    ids(&mut h, s.escaped.iter().map(|c| c.0));
    s.spawned_enemies.hash(&mut h);
    s.encounter_data.hash(&mut h);
    s.gold_proportion.hash(&mut h);
    sink(Part::Combat, h.finish());

    let mut h = Fnv::new();
    for t in RunRngType::ALL {
        let state = s.rng[t].state();
        state.counter.hash(&mut h);
        state.s.hash(&mut h);
    }
    sink(Part::Rng, h.finish());

    let p = &s.player;
    let mut h = Fnv::new();
    p.character.hash(&mut h);
    p.creature.0.hash(&mut h);
    p.energy.hash(&mut h);
    p.max_energy.hash(&mut h);
    p.gold.hash(&mut h);
    p.turn_number.hash(&mut h);
    p.extra_turn.hash(&mut h);
    p.phase.hash(&mut h);
    p.relics.hash(&mut h);
    p.potions.hash(&mut h);
    p.unported_potions.hash(&mut h);
    ids(&mut h, p.pets.iter().map(|c| c.0));
    p.orb_slots.hash(&mut h);
    p.orbs.hash(&mut h);
    p.stars.hash(&mut h);
    p.ready_to_end_turn.hash(&mut h);
    p.effect_depth.hash(&mut h);
    p.hooks_active.hash(&mut h);
    sink(Part::Player, h.finish());

    for c in &s.creatures {
        let mut h = Fnv::new();
        c.side.hash(&mut h);
        c.hp.hash(&mut h);
        c.max_hp.hash(&mut h);
        c.block.hash(&mut h);
        c.powers.len().hash(&mut h);
        for p in &c.powers {
            power(s, p, &mut h);
        }
        c.removed_powers.len().hash(&mut h);
        for p in &c.removed_powers {
            power(s, p, &mut h);
        }
        c.monster.hash(&mut h);
        c.pet_owner.map(|o| o.0).hash(&mut h);
        c.slot.hash(&mut h);
        c.infinite_hp_display.hash(&mut h);
        sink(Part::Creature(c.id.0), h.finish());
    }

    for pile in PileType::LISTENER_ORDER {
        let mut h = Fnv::new();
        ids(&mut h, s.player.pile(pile).iter().map(|c| c.0));
        sink(Part::Pile(pile), h.finish());
    }

    for card in &s.cards {
        let mut h = Fnv::new();
        card.kind.hash(&mut h);
        card.upgrade_level.hash(&mut h);
        card.keywords.hash(&mut h);
        card.energy_cost.hash(&mut h);
        card.star_cost.hash(&mut h);
        card.exhaust_on_next_play.hash(&mut h);
        card.single_turn_retain.hash(&mut h);
        card.single_turn_sly.hash(&mut h);
        card.affliction.hash(&mut h);
        card.enchantment.hash(&mut h);
        card.base_replay_count.hash(&mut h);
        card.data.hash(&mut h);
        card.is_dupe.hash(&mut h);
        card.clone_of.map(|c| c.0).hash(&mut h);
        card.deck_version().hash(&mut h);
        card.stale_calculated.hash(&mut h);
        s.cost_lists.energy(card.id).hash(&mut h);
        s.cost_lists.star(card.id).hash(&mut h);
        sink(Part::Card(card.id.0), h.finish());
    }

    let mut h = Fnv::new();
    s.history.hash(&mut h);
    sink(Part::History, h.finish());
}

/// A list of ids: its length, then each id.
fn ids(h: &mut Fnv, list: impl ExactSizeIterator<Item = u32>) {
    list.len().hash(h);
    for id in list {
        id.hash(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Character, Combat, CombatConfig};

    fn combat(seed: &str) -> Combat {
        Combat::new(CombatConfig::new(Character::Silent, seed, 0).expect("starter is ported"))
    }

    #[test]
    fn digest_is_a_function_of_the_state() {
        let a = combat("DIGEST");
        let b = combat("DIGEST");
        assert_eq!(canonical_digest(a.state()), canonical_digest(b.state()));
        let mut copy = combat("OTHER");
        copy.clone_from(&a);
        assert_eq!(canonical_digest(copy.state()), canonical_digest(a.state()));
        assert_ne!(
            canonical_digest(a.state()),
            canonical_digest(combat("OTHER").state())
        );
    }

    #[test]
    fn parts_fold_into_the_digest() {
        let mut c = combat("PARTS");
        let action = c.legal_actions().swap_remove(0);
        c.apply(action).expect("legal");
        let mut total = Fnv::new();
        for (_, part) in canonical_parts(c.state()) {
            total.write_u64(part);
        }
        assert_eq!(total.finish(), canonical_digest(c.state()));
    }

    #[test]
    fn every_step_changes_some_part() {
        let mut c = combat("STEP");
        let before = canonical_parts(c.state());
        c.apply(crate::Action::EndTurn).expect("legal");
        let after = canonical_parts(c.state());
        assert_ne!(before, after);
        assert!(before.iter().any(|(p, _)| *p == Part::Rng));
    }
}
