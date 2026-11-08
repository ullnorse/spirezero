//! The listener state's generation, the shared half of the extraction caches' keys.
//!
//! The values the caches hold (a hand card's modified cost, an attack intent's damage) are
//! folds over the value hooks' listeners. Every such listener in today's content is a power or
//! a relic, and what they read is their own fields (power amount and `_internalData`, relic
//! data), their owner, the card or attack in question, and whether the combat is in progress.
//! So the listener state is: per creature its liveness, side and powers (every [`Power`]
//! field and its data), the relics, the belt, and the combat's progress and side. (Not the
//! round: a hand card's cost is then still cached when the card comes back next turn.) It is
//! written out as exact words each observation; the generation moves when they differ from the
//! previous observation's, and a cache entry is valid only for the generation it was made in.
//!
//! [`Power`]: sts2_core::combat::Power

use sts2_core::combat::CombatState;

use crate::key::Words;

/// The generation of the listener state, bumped whenever it changes.
#[derive(Debug, Default)]
pub struct ListenerState {
    words: Vec<u64>,
    last: Vec<u64>,
    generation: u64,
}

impl ListenerState {
    /// Forgets the last state, so the next [`ListenerState::update`] starts a new generation.
    pub fn reset(&mut self) {
        self.last.clear();
        self.generation += 1;
    }

    /// The generation of `s`'s listener state (at least 1).
    pub fn update(&mut self, s: &CombatState) -> u64 {
        self.words.clear();
        let mut w = Words(&mut self.words);
        w.add(&s.in_progress);
        w.add(&s.side);
        for c in &s.creatures {
            w.add(&c.is_alive());
            w.add(&c.side);
            w.add(&c.powers.len());
            for power in &c.powers {
                w.add(power);
                w.add(s.power_data(power.id));
            }
        }
        w.add(&s.player.relics);
        w.add(&s.player.potions);
        if self.generation == 0 || self.words != self.last {
            self.generation += 1;
            std::mem::swap(&mut self.words, &mut self.last);
        }
        self.generation
    }
}
