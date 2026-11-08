//! The capacity survey (`docs/specs/rl-2x-plan.md`, stage 0): the high-water mark of every list
//! a fixed-capacity combat state would hold, over whatever combats a tool plays.
//!
//! [`HighWater::observe`] samples a combat between steps (after an `apply`), so a list that only
//! grows within a combat (cards, creatures, removed powers, this turn's history) is exact, while a
//! peak reached and undone inside one step is not seen: the draw pile while a shuffle refills it
//! and draws from it (bounded by [`Metric::DrawPlusDiscard`]), and the enemy turn's history, which
//! the end-turn step clears again. [`HighWater::observe_run`] samples a run's own lists between
//! rooms, for tools that never play the fights.

use std::fmt::{self, Write as _};

use crate::Combat;
use crate::combat::DecisionRequest;
use crate::combat::history::HistoryTag;

/// The history tags, to count this turn's entries.
const HISTORY_TAGS: [HistoryTag; 14] = [
    HistoryTag::CardPlayStarted,
    HistoryTag::CardPlayFinished,
    HistoryTag::CardDrawn,
    HistoryTag::CardExhausted,
    HistoryTag::CardAfflicted,
    HistoryTag::CardDiscarded,
    HistoryTag::CardGenerated,
    HistoryTag::EnergySpent,
    HistoryTag::BlockGained,
    HistoryTag::CreatureAttacked,
    HistoryTag::DamageReceived,
    HistoryTag::OrbChanneled,
    HistoryTag::PowerReceived,
    HistoryTag::StarsModified,
];

/// A surveyed list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Metric {
    /// Every combat card ever created (`CombatState::cards`).
    Cards,
    /// Every creature ever created (`CombatState::creatures`).
    Creatures,
    /// The enemies in combat (`CombatState::enemies`).
    Enemies,
    /// The allies in combat (`CombatState::allies`).
    Allies,
    /// The player's pets.
    Pets,
    /// One creature's powers.
    PowersPerCreature,
    /// All creatures' powers together.
    PowersTotal,
    /// One creature's removed powers.
    RemovedPerCreature,
    /// All creatures' removed powers together.
    RemovedTotal,
    /// The hand.
    Hand,
    /// The draw pile.
    Draw,
    /// The discard pile.
    Discard,
    /// The exhaust pile.
    Exhaust,
    /// The Play pile.
    Play,
    /// Draw and discard piles together: what a shuffle can put in the draw pile.
    DrawPlusDiscard,
    /// This turn's history entries (`CombatHistory::this_turn`).
    HistoryThisTurn,
    /// The player's relics.
    Relics,
    /// The belt's slots.
    PotionSlots,
    /// The channeled orbs.
    Orbs,
    /// The orb slots.
    OrbSlots,
    /// The run deck the combat started from.
    Deck,
    /// Cards with any cost modifier or temporary star cost (`cost_lists` entries).
    CostListCards,
    /// The longest one card's cost lists (energy plus star).
    CostListLen,
    /// Power instances (live or removed) with instance data (`power_data` entries).
    PowerData,
    /// A pending card choice's options.
    PendingOptions,
}

impl_index!(Metric);

impl Metric {
    /// Every metric, in declaration order.
    pub const ALL: [Self; 25] = [
        Self::Cards,
        Self::Creatures,
        Self::Enemies,
        Self::Allies,
        Self::Pets,
        Self::PowersPerCreature,
        Self::PowersTotal,
        Self::RemovedPerCreature,
        Self::RemovedTotal,
        Self::Hand,
        Self::Draw,
        Self::Discard,
        Self::Exhaust,
        Self::Play,
        Self::DrawPlusDiscard,
        Self::HistoryThisTurn,
        Self::Relics,
        Self::PotionSlots,
        Self::Orbs,
        Self::OrbSlots,
        Self::Deck,
        Self::CostListCards,
        Self::CostListLen,
        Self::PowerData,
        Self::PendingOptions,
    ];

    /// The metric's name in the survey's output.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cards => "cards",
            Self::Creatures => "creatures",
            Self::Enemies => "enemies",
            Self::Allies => "allies",
            Self::Pets => "pets",
            Self::PowersPerCreature => "powers_per_creature",
            Self::PowersTotal => "powers_total",
            Self::RemovedPerCreature => "removed_powers_per_creature",
            Self::RemovedTotal => "removed_powers_total",
            Self::Hand => "hand",
            Self::Draw => "draw",
            Self::Discard => "discard",
            Self::Exhaust => "exhaust",
            Self::Play => "play",
            Self::DrawPlusDiscard => "draw_plus_discard",
            Self::HistoryThisTurn => "history_this_turn",
            Self::Relics => "relics",
            Self::PotionSlots => "potion_slots",
            Self::Orbs => "orbs",
            Self::OrbSlots => "orb_slots",
            Self::Deck => "deck",
            Self::CostListCards => "cost_list_cards",
            Self::CostListLen => "cost_list_len",
            Self::PowerData => "power_data",
            Self::PendingOptions => "pending_options",
        }
    }

    /// The metric named `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }
}

/// The capacities whose spill rate the survey counts: [`Mark::over`].
pub const THRESHOLDS: [usize; 6] = [8, 16, 32, 64, 128, 256];

/// One metric's high-water mark and the first combat that reached it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mark {
    /// The highest value seen.
    pub max: usize,
    /// How many sampled states held more than each of [`THRESHOLDS`]: the states a list of that
    /// capacity would have spilled in.
    pub over: [u64; THRESHOLDS.len()],
    /// Character, encounter, ascension, round and the caller's label; empty while `max` is 0.
    pub at: String,
}

/// The high-water marks of every [`Metric`], and how many states were sampled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighWater {
    marks: [Mark; Metric::ALL.len()],
    /// How many states were sampled.
    pub samples: u64,
}

impl Default for HighWater {
    fn default() -> Self {
        Self {
            marks: std::array::from_fn(|_| Mark::default()),
            samples: 0,
        }
    }
}

impl Mark {
    /// Counts a sampled value against the thresholds.
    fn count(&mut self, value: usize) {
        for (o, t) in self.over.iter_mut().zip(THRESHOLDS) {
            *o += u64::from(value > t);
        }
    }
}

impl HighWater {
    /// The mark of `metric`.
    #[must_use]
    pub const fn mark(&self, metric: Metric) -> &Mark {
        &self.marks[metric.index()]
    }

    /// Samples `combat`; `label` (a seed, a run) is called only when a mark rises.
    pub fn observe(&mut self, combat: &Combat, label: impl Fn() -> String) {
        let s = combat.state();
        let p = &s.player;
        self.samples += 1;
        let mut values = [0; Metric::ALL.len()];
        let mut set = |m: Metric, v: usize| values[m.index()] = v;
        set(Metric::Cards, s.cards.len());
        set(Metric::Creatures, s.creatures.len());
        set(Metric::Enemies, s.enemies.len());
        set(Metric::Allies, s.allies.len());
        set(Metric::Pets, p.pets.len());
        let creature_max =
            |f: fn(&crate::combat::Creature) -> usize| s.creatures.iter().map(f).max().unwrap_or(0);
        let creature_sum =
            |f: fn(&crate::combat::Creature) -> usize| s.creatures.iter().map(f).sum::<usize>();
        set(Metric::PowersPerCreature, creature_max(|c| c.powers.len()));
        set(Metric::PowersTotal, creature_sum(|c| c.powers.len()));
        set(
            Metric::RemovedPerCreature,
            creature_max(|c| c.removed_powers.len()),
        );
        set(
            Metric::RemovedTotal,
            creature_sum(|c| c.removed_powers.len()),
        );
        set(Metric::Hand, p.hand.len());
        set(Metric::Draw, p.draw.len());
        set(Metric::Discard, p.discard.len());
        set(Metric::Exhaust, p.exhaust.len());
        set(Metric::Play, p.play.len());
        set(Metric::DrawPlusDiscard, p.draw.len() + p.discard.len());
        set(
            Metric::HistoryThisTurn,
            HISTORY_TAGS
                .into_iter()
                .map(|t| s.history_this_turn(t).len())
                .sum(),
        );
        set(Metric::Relics, p.relics.len());
        set(Metric::PotionSlots, p.potions.len());
        set(Metric::Orbs, p.orbs.len());
        set(Metric::OrbSlots, usize::from(p.orb_slots));
        set(Metric::Deck, s.deck.len());
        let cost_lens = s
            .cards
            .iter()
            .map(|c| s.cost_lists.energy(c.id).len() + s.cost_lists.star(c.id).len());
        set(
            Metric::CostListCards,
            cost_lens.clone().filter(|&n| n > 0).count(),
        );
        set(Metric::CostListLen, cost_lens.max().unwrap_or(0));
        set(
            Metric::PowerData,
            s.creatures
                .iter()
                .flat_map(|c| c.powers.iter().chain(&c.removed_powers))
                .filter(|pw| *s.power_data(pw.id) != crate::content::powers::PowerData::None)
                .count(),
        );
        set(
            Metric::PendingOptions,
            combat
                .pending_decision()
                .map_or(0, |DecisionRequest::ChooseCards { options, .. }| {
                    options.len()
                }),
        );
        for (mark, value) in self.marks.iter_mut().zip(values) {
            mark.count(value);
            if value > mark.max {
                mark.max = value;
                mark.at = format!(
                    "{} {} A{} round {} {}",
                    p.character.name(),
                    s.encounter.entry(),
                    s.ascension,
                    s.round,
                    label()
                );
            }
        }
    }

    /// Samples a run between rooms (the history replay never plays a fight): its deck, relics
    /// and potion slots, as [`Metric::Deck`], [`Metric::Relics`] and [`Metric::PotionSlots`].
    pub fn observe_run(&mut self, s: &crate::run::RunState, label: impl Fn() -> String) {
        self.samples += 1;
        for (metric, value) in [
            (Metric::Deck, s.deck.len()),
            (Metric::Relics, s.relics.len()),
            (Metric::PotionSlots, s.potions.len()),
        ] {
            let mark = &mut self.marks[metric.index()];
            mark.count(value);
            if value > mark.max {
                mark.max = value;
                mark.at = format!(
                    "{} run A{} act {} {}",
                    s.character.name(),
                    s.ascension,
                    s.act_index + 1,
                    label()
                );
            }
        }
    }

    /// Takes the higher of each mark (ties keep `self`'s place).
    pub fn merge(&mut self, other: &Self) {
        self.samples += other.samples;
        for (mark, theirs) in self.marks.iter_mut().zip(&other.marks) {
            for (o, t) in mark.over.iter_mut().zip(theirs.over) {
                *o += t;
            }
            if theirs.max > mark.max {
                mark.max = theirs.max;
                mark.at.clone_from(&theirs.at);
            }
        }
    }

    /// One `name max over... at` line per metric and a `samples N` line: the form [`HighWater::parse`]
    /// reads back (to merge the marks of several processes).
    #[must_use]
    pub fn to_lines(&self) -> String {
        let mut out = format!("samples {}\n", self.samples);
        for m in Metric::ALL {
            let mark = self.mark(m);
            let _ = write!(out, "{} {}", m.name(), mark.max);
            for o in mark.over {
                let _ = write!(out, " {o}");
            }
            let _ = writeln!(out, " {}", mark.at);
        }
        out
    }

    /// Reads [`HighWater::to_lines`]'s output; unknown lines are skipped.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut hw = Self::default();
        for line in text.lines() {
            let mut words = line.split(' ');
            let (Some(name), Some(Ok(value))) = (words.next(), words.next().map(str::parse::<u64>))
            else {
                continue;
            };
            if name == "samples" {
                hw.samples = value;
            } else if let Some(m) = Metric::from_name(name) {
                let mark = &mut hw.marks[m.index()];
                mark.max = usize::try_from(value).unwrap_or(usize::MAX);
                for o in &mut mark.over {
                    *o = words.next().and_then(|w| w.parse().ok()).unwrap_or(0);
                }
                mark.at = words.collect::<Vec<_>>().join(" ");
            }
        }
        hw
    }
}

/// A markdown table: metric, high-water mark, the share of sampled states over each threshold
/// (parts per million), where the mark was first reached.
impl fmt::Display for HighWater {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "| list | max |")?;
        for t in THRESHOLDS {
            write!(f, " ppm > {t} |")?;
        }
        writeln!(f, " first reached at |")?;
        writeln!(f, "|---|---:|{}---|", "---:|".repeat(THRESHOLDS.len()))?;
        for m in Metric::ALL {
            let mark = self.mark(m);
            write!(f, "| {} | {} |", m.name(), mark.max)?;
            for o in mark.over {
                write!(
                    f,
                    " {} |",
                    u128::from(o) * 1_000_000 / u128::from(self.samples.max(1))
                )?;
            }
            writeln!(f, " {} |", mark.at)?;
        }
        write!(f, "({} states sampled)", self.samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Character, CombatConfig};

    #[test]
    fn marks_rise_merge_and_round_trip() {
        let mut combat =
            Combat::new(CombatConfig::new(Character::Ironclad, "HW", 0).expect("ported"));
        let mut hw = HighWater::default();
        hw.observe(&combat, || "start".to_owned());
        assert_eq!(hw.mark(Metric::Hand).max, 5);
        assert!(hw.mark(Metric::Hand).at.ends_with("start"));
        assert_eq!(hw.mark(Metric::Cards).max, 10);
        combat.apply(crate::Action::EndTurn).expect("legal");
        let mut later = HighWater::default();
        later.observe(&combat, String::new);
        hw.merge(&later);
        assert_eq!(hw.samples, 2);
        assert_eq!(HighWater::parse(&hw.to_lines()), hw);
    }
}
