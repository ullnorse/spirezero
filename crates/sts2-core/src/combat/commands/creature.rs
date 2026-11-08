//! `CreatureCmd.Add` (`CreatureCmd.cs:38-80`): a monster joining combat mid-fight, and
//! `CreatureCmd.Escape` (`CreatureCmd.cs:609-632`): a monster leaving it alive.

use crate::combat::engine::{Engine, Event, Flow};
use crate::combat::state::{CreatureId, Side};
use crate::content::monsters::{self, MonsterKind};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

impl Engine<'_> {
    /// `CreatureCmd.Add<T>(combatState, slotName)` for an enemy: [`Engine::create_enemy`], then
    /// [`Engine::add_created_enemy`]. Combat must be in progress.
    pub(crate) fn add_enemy(
        &mut self,
        kind: MonsterKind,
        slot: Option<&'static str>,
    ) -> Flow<CreatureId> {
        let id = self.create_enemy(kind, slot);
        self.add_created_enemy(id)?;
        Ok(id)
    }

    /// `CombatState.CreateCreature(monster, Enemy, slot)` (`CombatState.cs:233-248`): the creature
    /// exists (HP rolled on `Rng.Niche` against the enemies in combat now, its `CombatId`, its
    /// `Rng`, `SpawnedEnemies`) but is not in combat until [`Engine::add_created_enemy`]. Powers
    /// may be applied to it meanwhile (`SurprisePower` gives the fat gremlin its `HeistPower`s
    /// before adding it).
    pub(crate) fn create_enemy(
        &mut self,
        kind: MonsterKind,
        slot: Option<&'static str>,
    ) -> CreatureId {
        crate::combat::create_enemy(self.s, kind, slot)
    }

    /// `CreatureCmd.Add(creature)` for an enemy [`Engine::create_enemy`] made: `AddCreature`
    /// (then `SortEnemiesBySlotName` if it has a slot), `AfterCreatureAdded` (its
    /// `AfterAddedToRoom`, and its first roll during the player's turn), then
    /// `Hook.AfterCreatureAddedToCombat`. `PrepareForNextTurn(rollNewMove: false)` only refreshes
    /// intents.
    ///
    /// # Panics
    /// If combat is not in progress (`CreatureCmd.Add` throws).
    pub(crate) fn add_created_enemy(&mut self, id: CreatureId) -> Flow {
        assert!(self.s.in_progress, "a creature is added outside of combat");
        debug_assert!(
            !self.s.contains_creature(id),
            "the creature is already in combat"
        );
        self.s.push_enemy(id);
        if self.s.creature(id).slot.is_some() {
            #[cfg(debug_assertions)]
            let before = self.s.enemies.clone();
            self.s.sort_enemies_by_slot_name();
            // `creature_added` only places the new enemy. The 3-item `SwapIfGreater` is not
            // stable: two enemies sharing a slot index above the new one's would swap. The
            // enemies are always sorted, and only Fogmog's and The Obscura's "illusion" (their
            // lowest slot) and Axebot's "front" (its only slot) are ever shared, so no encounter
            // gets there.
            #[cfg(debug_assertions)]
            debug_assert!(
                before
                    .iter()
                    .filter(|&&e| e != id)
                    .eq(self.s.enemies.iter().filter(|&&e| e != id)),
                "sorting by slot reordered the enemies already in combat"
            );
        }
        monsters::after_added_to_room(self, id)?;
        if self.s.side == Side::Player {
            monsters::roll_move(self, id);
        }
        if self.s.modifiers.murderous {
            // `Murderous.AfterCreatureAddedToCombat`: late enemies get the Strength too.
            self.apply_power(id, PowerKind::Strength, Decimal::from(3), None)?;
        }
        self.after_creature_added_to_combat(id)
    }

    /// `CreatureCmd.Escape(creature)`: a living creature in a live combat loses every power
    /// without callbacks (`RemoveAllPowersInternalExcept`: `RemoveInternal` raises no hook),
    /// leaves the room (`CombatManager.RemoveCreature`: `BeforeRemovedFromRoom`, the state machine
    /// dropped, no more listening) and is recorded in `EscapedCreatures`
    /// (`CombatState.CreatureEscaped`, `CombatState.cs:267-271`). It is not killed: no death hooks,
    /// and it counts against the gold reward (`CalculateGoldProportion`). The win condition is the
    /// caller's to check (a monster's move ends with the turn loop's check).
    pub(crate) fn escape(&mut self, id: CreatureId) {
        // `creature.CombatState == null`: the creature already left (escaped or removed).
        if self.s.creature(id).is_dead() || !self.s.in_progress || !self.s.contains_creature(id) {
            return;
        }
        self.s.remove_all_powers(id);
        if self.s.creature(id).monster.is_some() {
            monsters::before_removed_from_room(self, id);
        }
        self.s.escaped.push(id);
        self.s.remove_enemy(id);
        self.s.remove_ally(id);
        self.emit(Event::Escaped(id));
    }
}
