//! Orbs: `OrbCmd` (`Commands/OrbCmd.cs`), the `OrbQueue` triggers (`Entities.Orbs/OrbQueue.cs`)
//! and the five orb models (`Models.Orbs/*.cs`, `Models/OrbModel.cs`). Spec: `docs/specs/orbs.md`.
//!
//! No orb overrides a combat hook, so orbs are not hook listeners; an orb is plain data on
//! [`Player::orbs`](crate::combat::state::Player). An orb taken out of the queue keeps working
//! from the copy its caller holds (a trigger over the queue snapshot, an evoke that dequeued).

use crate::combat::engine::{Engine, Flow, ValueProps};
use crate::combat::history::HistoryKind;
use crate::combat::hooks::Hooked;
use crate::combat::state::{CombatState, CreatureId, Orb, OrbId, OrbKind};
use crate::decimal::Decimal;
use crate::rng::RunRngType;

/// `OrbQueue.maxCapacity`.
const MAX_ORB_SLOTS: i32 = 10;

impl Hooked<'_> {
    /// `OrbModel.ModifyOrbValue(value)`: [`Hooked::modify_orb_value`].
    fn orb_value(self, orb: Orb, value: i32) -> Decimal {
        self.modify_orb_value(orb, Decimal::from(value))
    }

    /// `PassiveVal`: Lightning `Mod(3)`, Frost `Mod(2)`, Dark `Mod(6)`, Plasma 1, Glass
    /// `Mod(_passiveVal)`.
    pub(crate) fn orb_passive_val(self, orb: Orb) -> Decimal {
        match orb.kind {
            OrbKind::Lightning => self.orb_value(orb, 3),
            OrbKind::Frost => self.orb_value(orb, 2),
            OrbKind::Dark => self.orb_value(orb, 6),
            OrbKind::Plasma => Decimal::ONE,
            OrbKind::Glass => self.modify_orb_value(orb, orb.value),
        }
    }

    /// `EvokeVal`: Lightning `Mod(8)`, Frost `Mod(5)`, Dark `_evokeVal` (not modified), Plasma 2,
    /// Glass `PassiveVal * 2`.
    pub(crate) fn orb_evoke_val(self, orb: Orb) -> Decimal {
        match orb.kind {
            OrbKind::Lightning => self.orb_value(orb, 8),
            OrbKind::Frost => self.orb_value(orb, 5),
            OrbKind::Dark => orb.value,
            OrbKind::Plasma => Decimal::from(2),
            OrbKind::Glass => self.orb_passive_val(orb) * Decimal::from(2),
        }
    }
}

impl CombatState {
    /// The orb's `PassiveVal` now.
    #[must_use]
    pub fn orb_passive_val(&self, orb: Orb) -> Decimal {
        self.hooked().orb_passive_val(orb)
    }

    /// The orb's `EvokeVal` now.
    #[must_use]
    pub fn orb_evoke_val(&self, orb: Orb) -> Decimal {
        self.hooked().orb_evoke_val(orb)
    }

    /// The queued orb with `id`, if it is still in the queue.
    fn queued_orb(&self, id: OrbId) -> Option<Orb> {
        self.player.orbs.iter().copied().find(|o| o.id == id)
    }
}

impl Engine<'_> {
    /// `OrbModel.GetRandomOrb(rng)` on `Rng.CombatOrbs` (`OrbModel.cs:169`): one `NextItem` over
    /// the five orb kinds.
    pub(crate) fn random_orb(&mut self) -> OrbKind {
        *self.s.rng[RunRngType::CombatOrbs]
            .next_item(&OrbKind::ALL)
            .expect("five orb kinds")
    }

    /// `OrbCmd.AddSlots` (`OrbCmd.cs:23-33`): nothing once combat is over or ending; else
    /// `min(10 - Capacity, amount)` more slots.
    pub(crate) fn add_orb_slots(&mut self, amount: i32) {
        if self.hooked().is_over_or_ending() {
            return;
        }
        let slots = i32::from(self.s.player.orb_slots);
        let amount = (MAX_ORB_SLOTS - slots).min(amount);
        self.s.player.orb_slots = u8::try_from(slots + amount).expect("0..=10 orb slots");
    }

    /// `OrbCmd.RemoveSlots` (`OrbCmd.cs:41-49`, `OrbQueue.RemoveCapacity`): nothing once combat
    /// is over or ending; else `min(Capacity, amount)` fewer slots, dropping the last orbs that
    /// no longer fit (no evoke, no hooks).
    pub(crate) fn remove_orb_slots(&mut self, amount: i32) {
        if self.hooked().is_over_or_ending() {
            return;
        }
        let slots = i32::from(self.s.player.orb_slots);
        let remaining = (slots - slots.min(amount)).max(0);
        let player = &mut self.s.player;
        player.orb_slots = u8::try_from(remaining).expect("0..=10 orb slots");
        player.orbs.truncate(usize::from(player.orb_slots));
    }

    /// `OrbCmd.Channel` (`OrbCmd.cs:68-92`): nothing once combat is over or ending; a character
    /// without base slots gets its first slot; a full queue evokes its front orb first. Only an
    /// orb that joined the queue (`TryEnqueue` fails with no slots) is logged
    /// (`History.OrbChanneled`) and fires `AfterOrbChanneled`.
    pub(crate) fn channel(&mut self, kind: OrbKind) -> Flow {
        if self.hooked().is_over_or_ending() {
            return Ok(());
        }
        if self.s.player.character.base_orb_slots() == 0 && self.s.player.orb_slots == 0 {
            self.add_orb_slots(1);
        }
        let orb = Orb {
            id: OrbId(self.s.next_orb_id),
            kind,
            value: kind.initial_value(),
        };
        self.s.next_orb_id += 1;
        if self.s.player.orbs.len() >= usize::from(self.s.player.orb_slots) {
            self.evoke_next(true)?;
        }
        // OrbQueue.TryEnqueue.
        if self.s.player.orb_slots == 0 {
            return Ok(());
        }
        assert!(
            self.s.player.orbs.len() < usize::from(self.s.player.orb_slots),
            "OrbQueue is full"
        );
        self.s.player.orbs.push(orb);
        let player = self.s.player.creature;
        self.record(player, HistoryKind::OrbChanneled(kind));
        self.after_orb_channeled(orb)
    }

    /// `OrbCmd.EvokeNext(player, dequeue)` (`OrbCmd.cs:94-104`): evokes the front orb, if any.
    pub(crate) fn evoke_next(&mut self, dequeue: bool) -> Flow {
        match self.s.player.orbs.first() {
            Some(&orb) => self.evoke(orb, dequeue),
            None => Ok(()),
        }
    }

    /// `OrbCmd.EvokeLast(player, dequeue)` (`OrbCmd.cs:106-116`): evokes the last orb, if any.
    pub(crate) fn evoke_last(&mut self, dequeue: bool) -> Flow {
        match self.s.player.orbs.last() {
            Some(&orb) => self.evoke(orb, dequeue),
            None => Ok(()),
        }
    }

    /// `OrbCmd.Evoke` (`OrbCmd.cs:125-153`): nothing once combat is over or ending or with an
    /// empty queue; with `dequeue` the orb leaves the queue first. Then the orb's `Evoke` and,
    /// while the owner is in combat, `AfterOrbEvoked` with the creatures it affected.
    fn evoke(&mut self, orb: Orb, dequeue: bool) -> Flow {
        if self.hooked().is_over_or_ending() || self.s.player.orbs.is_empty() {
            return Ok(());
        }
        if dequeue {
            self.s.player.orbs.retain(|o| o.id != orb.id);
        }
        let targets = self.orb_evoke(orb)?;
        if self.s.contains_creature(self.s.player.creature) {
            self.after_orb_evoked(orb, &targets)?;
        }
        Ok(())
    }

    /// `OrbCmd.Passive(orb, target, countAffectedByHooks)` (`OrbCmd.cs:155-170`): nothing once
    /// combat is over or ending; the raw `Passive` (bypassing `ModifyOrbPassiveTriggerCount`),
    /// or `TriggerPassive` when `count_affected_by_hooks`.
    pub(crate) fn orb_passive(
        &mut self,
        orb: Orb,
        target: Option<CreatureId>,
        count_affected_by_hooks: bool,
    ) -> Flow {
        if self.hooked().is_over_or_ending() {
            return Ok(());
        }
        if count_affected_by_hooks {
            self.trigger_orb_passive(orb, target)
        } else {
            self.orb_passive_once(orb, target).map(|_| ())
        }
    }

    /// `OrbQueue.AfterTurnStart` (`OrbQueue.cs:92-102`, `CombatManager.cs:783-796`): over a
    /// snapshot of the queue, while the owner is in combat, each orb's
    /// `AfterTurnStartOrbTrigger` (Plasma's `TriggerPassive`).
    pub(crate) fn orbs_after_turn_start(&mut self) -> Flow {
        for orb in self.s.player.orbs.clone() {
            if !self.s.contains_creature(self.s.player.creature) {
                return Ok(());
            }
            if orb.kind == OrbKind::Plasma {
                self.trigger_orb_passive(orb, None)?;
            }
        }
        Ok(())
    }

    /// `OrbQueue.BeforeTurnEnd` (`OrbQueue.cs:80-90`, `CombatManager.cs:1601`): over a snapshot
    /// of the queue, oldest first, while the owner is in combat, each orb's
    /// `BeforeTurnEndOrbTrigger` (`TriggerPassive` for every kind but Plasma). Not guarded:
    /// orbs after a passive that won the combat still trigger.
    pub(crate) fn orbs_before_turn_end(&mut self) -> Flow {
        for orb in self.s.player.orbs.clone() {
            if !self.s.contains_creature(self.s.player.creature) {
                return Ok(());
            }
            if orb.kind != OrbKind::Plasma {
                self.trigger_orb_passive(orb, None)?;
            }
        }
        Ok(())
    }

    /// `OrbModel.TriggerPassive` (`OrbModel.cs:246-262`): `ModifyOrbPassiveTriggerCount(1)`,
    /// `AfterModifyingOrbPassiveTriggerCount` to the changers, then `Passive` that many times.
    fn trigger_orb_passive(&mut self, orb: Orb, target: Option<CreatureId>) -> Flow {
        let (count, modifiers) = self.hooked().modify_orb_passive_trigger_count(orb, 1);
        self.after_modifying_orb_passive_trigger_count(orb, &modifiers)?;
        let mut orb = orb;
        for _ in 0..count {
            orb = self.orb_passive_once(orb, target)?;
        }
        Ok(())
    }

    /// The orb's own `Passive(target)`. `orb` is the caller's copy; the queued orb, while it is
    /// queued, is the live one. Returns the orb after the passive, for a repeat.
    fn orb_passive_once(&mut self, orb: Orb, target: Option<CreatureId>) -> Flow<Orb> {
        let mut orb = self.s.queued_orb(orb.id).unwrap_or(orb);
        let player = self.s.player.creature;
        match orb.kind {
            // `LightningOrb.Passive` (`LightningOrb.cs:32-36`).
            OrbKind::Lightning => {
                let value = self.hooked().orb_passive_val(orb);
                self.lightning_damage(value, target)?;
            }
            // `FrostOrb.Passive` (`FrostOrb.cs:30-50`); the `HibernatePower` branch is
            // multiplayer only.
            OrbKind::Frost => {
                assert!(target.is_none(), "Frost orbs cannot target creatures.");
                let value = self.hooked().orb_passive_val(orb);
                self.gain_block(player, value, ValueProps::UNPOWERED, None)?;
            }
            // `DarkOrb.Passive` (`DarkOrb.cs:30-40`): `_evokeVal += PassiveVal`.
            OrbKind::Dark => {
                assert!(target.is_none(), "Dark orbs cannot target creatures.");
                orb.value += self.hooked().orb_passive_val(orb);
                self.write_orb(orb);
            }
            // `PlasmaOrb.Passive` (`PlasmaOrb.cs:28-36`).
            OrbKind::Plasma => {
                assert!(target.is_none(), "Plasma orbs cannot target creatures.");
                let value = self.hooked().orb_passive_val(orb);
                self.gain_energy(value)?;
            }
            // `GlassOrb.Passive` (`GlassOrb.cs:27-38`): skipped at a value of 0 or less; else
            // `_passiveVal` drops by 1 (floor 0) before the damage to every hittable enemy.
            OrbKind::Glass => {
                let targets = self.hooked().hittable_enemies();
                let value = self.hooked().orb_passive_val(orb);
                if value > Decimal::ZERO {
                    orb.value = (orb.value - Decimal::ONE).max(Decimal::ZERO);
                    self.write_orb(orb);
                    self.damage(&targets, value, ValueProps::UNPOWERED, Some(player), None)?;
                }
            }
        }
        Ok(orb)
    }

    /// The orb's own `Evoke`; returns the creatures it affected.
    fn orb_evoke(&mut self, orb: Orb) -> Flow<Vec<CreatureId>> {
        let player = self.s.player.creature;
        match orb.kind {
            // `LightningOrb.Evoke` (`LightningOrb.cs:38-41`).
            OrbKind::Lightning => {
                let value = self.hooked().orb_evoke_val(orb);
                self.lightning_damage(value, None)
            }
            // `FrostOrb.Evoke` (`FrostOrb.cs:52-69`), single player.
            OrbKind::Frost => {
                let value = self.hooked().orb_evoke_val(orb);
                self.gain_block(player, value, ValueProps::UNPOWERED, None)?;
                Ok(vec![player])
            }
            // `DarkOrb.Evoke` (`DarkOrb.cs:42-56`): the hittable enemy with the lowest HP (the
            // first on ties).
            OrbKind::Dark => {
                let hittable = self.hooked().hittable_enemies();
                let Some(&weakest) = hittable.iter().min_by_key(|&&e| self.s.creature(e).hp) else {
                    return Ok(Vec::new());
                };
                let value = self.hooked().orb_evoke_val(orb);
                self.damage(&[weakest], value, ValueProps::UNPOWERED, Some(player), None)?;
                Ok(vec![weakest])
            }
            // `PlasmaOrb.Evoke` (`PlasmaOrb.cs:38-42`).
            OrbKind::Plasma => {
                let value = self.hooked().orb_evoke_val(orb);
                self.gain_energy(value)?;
                Ok(vec![player])
            }
            // `GlassOrb.Evoke` (`GlassOrb.cs:40-53`): every hittable enemy, nothing at 0 or less.
            OrbKind::Glass => {
                let enemies = self.hooked().hittable_enemies();
                if self.hooked().orb_evoke_val(orb) <= Decimal::ZERO {
                    return Ok(Vec::new());
                }
                let value = self.hooked().orb_evoke_val(orb);
                self.damage(&enemies, value, ValueProps::UNPOWERED, Some(player), None)?;
                Ok(enemies.into_vec())
            }
        }
    }

    /// `LightningOrb.ApplyLightningDamage` (`LightningOrb.cs:43-64`): `target`, or one
    /// `NextItem` on `Rng.CombatTargets` over the hittable opponents (even just one); nothing
    /// and no draw when none is hittable.
    fn lightning_damage(
        &mut self,
        value: Decimal,
        target: Option<CreatureId>,
    ) -> Flow<Vec<CreatureId>> {
        let hittable = self.hooked().hittable_enemies();
        if hittable.is_empty() {
            return Ok(Vec::new());
        }
        let target = target
            .or_else(|| self.random_target(&hittable))
            .expect("a hittable opponent");
        let player = self.s.player.creature;
        self.damage(&[target], value, ValueProps::UNPOWERED, Some(player), None)?;
        Ok(vec![target])
    }

    /// Writes `orb`'s field back to the queued orb, if it is still queued.
    fn write_orb(&mut self, orb: Orb) {
        if let Some(queued) = self.s.player.orbs.iter_mut().find(|o| o.id == orb.id) {
            *queued = orb;
        }
    }
}
