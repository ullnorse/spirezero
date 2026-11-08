//! Encounters: one file per encounter, each a unit struct implementing [`EncounterModel`].
//!
//! To add an encounter: its file `<name>.rs` declaring `pub struct <Name>`; `build.rs` generates
//! the variant, `ALL`, `id` and `model` (see `docs/specs/content-api.md`, "Registration").

include!(concat!(env!("OUT_DIR"), "/content/encounters.rs"));

use crate::combat::CombatState;
use crate::content::monsters::MonsterKind;
use crate::rng::{Rng, RunRngSet, xxh64};
use crate::run::odds::GoldProportion;

/// An encounter instance's own fields (`CombatState::encounter_data`); add a variant for an
/// encounter that keeps state during the fight (`GremlinMercNormal.GoldWasStolen`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EncounterData {
    /// No fields.
    #[default]
    None,
    /// `GremlinMercNormal`.
    GremlinMercNormal {
        /// `GoldWasStolen`.
        gold_was_stolen: bool,
    },
    /// `BattlewornDummyEventEncounter` (V1-V3).
    BattlewornDummy {
        /// `RanOutOfTime`.
        ran_out_of_time: bool,
    },
}

/// An encounter model (`EncounterModel`).
pub(crate) trait EncounterModel {
    /// `GenerateMonsters`: the monsters in spawn order, with their fields set. `rng` is the
    /// encounter's own `Rng`.
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind>;
    /// `Slots`: the slot names in enemy order (`SortEnemiesBySlotName`).
    fn slots(&self) -> &'static [&'static str] {
        &[]
    }
    /// The slot name `GenerateMonsters` pairs with each monster, by spawn index; empty for an
    /// encounter without slots. Every slotted encounter in scope spawns a fixed list.
    fn spawn_slots(&self) -> &'static [&'static str] {
        &[]
    }
    /// `CalculateGoldProportion(combatState)`, at combat end: by default the share of the spawned
    /// monster models that did not escape (`EncounterModel.cs:368-371`).
    fn calculate_gold_proportion(&self, s: &CombatState) -> GoldProportion {
        GoldProportion::from_escaped(s.escaped.len(), s.spawned_enemies.len())
    }
    /// An override of `MinGoldReward` and `MaxGoldReward`, which replaces the room type's
    /// default range (and its `Poverty` scaling, `EncounterModel.cs:64-100`); `None` keeps it.
    fn gold_reward(&self) -> Option<(i32, i32)> {
        None
    }
    /// `ShouldGiveRewards` (`EncounterModel.cs:62`): false for an encounter whose won room offers
    /// no rewards screen at all (`NCombatUi.OnCombatWon` → `ProceedWithoutRewards`).
    fn should_give_rewards(&self) -> bool {
        true
    }
}

impl Encounter {
    /// `ModelId.Entry` ([`Self::id`]).
    #[must_use]
    pub const fn entry(self) -> &'static str {
        self.id()
    }

    /// The encounter with `ModelId.Entry` `entry` ([`Self::from_id`]).
    #[must_use]
    pub fn from_entry(entry: &str) -> Option<Self> {
        Self::from_id(entry)
    }

    /// `Slots`.
    #[must_use]
    pub fn slots(self) -> &'static [&'static str] {
        with_model!(self, m => m.slots())
    }

    /// `CalculateGoldProportion(combatState)`.
    #[must_use]
    pub fn calculate_gold_proportion(self, s: &CombatState) -> GoldProportion {
        with_model!(self, m => m.calculate_gold_proportion(s))
    }

    /// The `MinGoldReward`/`MaxGoldReward` override, if the encounter has one.
    #[must_use]
    pub fn gold_reward(self) -> Option<(i32, i32)> {
        with_model!(self, m => m.gold_reward())
    }

    /// `ShouldGiveRewards`.
    #[must_use]
    pub fn should_give_rewards(self) -> bool {
        with_model!(self, m => m.should_give_rewards())
    }

    /// `Encounter.GetNextSlot` (`EncounterModel.cs:240-243`): the first slot no enemy in combat
    /// holds; `None` (the C# `string.Empty`) when every slot is taken.
    #[must_use]
    pub fn next_slot(self, s: &CombatState) -> Option<&'static str> {
        self.slots()
            .iter()
            .copied()
            .find(|&slot| s.enemies.iter().all(|&e| s.creature(e).slot != Some(slot)))
    }

    /// `GenerateMonstersWithSlots` (`EncounterModel.cs:254-272`): seeds the encounter's `Rng`
    /// with `run seed + TotalFloor + xxh64(Entry)` and runs `GenerateMonsters`; each monster
    /// comes with its slot name, if any.
    #[must_use]
    pub fn generate_monsters(
        self,
        run_rng: &RunRngSet,
        total_floor: i32,
    ) -> Vec<(MonsterKind, Option<&'static str>)> {
        let seed = run_rng
            .seed()
            .cast_signed()
            .wrapping_add(i64::from(total_floor))
            .cast_unsigned()
            .wrapping_add(xxh64(self.entry()));
        let mut rng = Rng::new(seed);
        let m = self.model();
        let slots = m.spawn_slots();
        m.generate_monsters(&mut rng)
            .into_iter()
            .enumerate()
            .map(|(i, kind)| (kind, slots.get(i).copied()))
            .collect()
    }
}
