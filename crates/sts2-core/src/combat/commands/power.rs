//! `PowerCmd.Apply`, `PowerCmd.ModifyAmount` and `PowerCmd.Remove` (`PowerCmd.cs:20-330`).

use crate::combat::engine::{Engine, Event, Flow};
use crate::combat::history::HistoryKind;
use crate::combat::hooks::{Listener, PowerQuery, PowerRef};
use crate::combat::state::{CardId, CreatureId, Power, PowerId, Side};
use crate::content::powers::{PowerInstanceType, PowerKind, PowerType};
use crate::decimal::Decimal;

impl Engine<'_> {
    /// `PowerCmd.Apply<T>(targets, amount, applier)`: [`Engine::apply_power`] per target.
    pub(crate) fn apply_power_to_all(
        &mut self,
        targets: &[CreatureId],
        kind: PowerKind,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        self.apply_power_to_all_from_card(None, targets, kind, amount, applier)
    }

    /// [`Engine::apply_power_to_all`] with a `cardSource`.
    pub(crate) fn apply_power_to_all_from_card(
        &mut self,
        card: Option<CardId>,
        targets: &[CreatureId],
        kind: PowerKind,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        for &target in targets {
            self.apply_power_from_card(card, target, kind, amount, applier)?;
        }
        Ok(())
    }

    /// `PowerCmd.Apply<T>(target, amount, applier)` (`PowerCmd.cs:105-160`): stacks into the
    /// instance `FindExistingInstanceForStacking` finds, else adds a new one.
    pub(crate) fn apply_power(
        &mut self,
        target: CreatureId,
        kind: PowerKind,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        self.apply_power_instance(target, kind, amount, applier)
            .map(drop)
    }

    /// [`Engine::apply_power`] with a `cardSource`: the card that applies the power.
    pub(crate) fn apply_power_from_card(
        &mut self,
        card: Option<CardId>,
        target: CreatureId,
        kind: PowerKind,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        self.apply_power_instance_from_card(card, target, kind, amount, applier)
            .map(drop)
    }

    /// [`Engine::apply_power`], returning the instance as `PowerCmd.Apply<T>` does
    /// (`PowerCmd.cs:38-60`): the new instance (`None` if it is not on `target` afterwards,
    /// where the C# returns a detached one), or the stacked one unless `ModifyAmount` returned
    /// a new amount of 0.
    pub(crate) fn apply_power_instance(
        &mut self,
        target: CreatureId,
        kind: PowerKind,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow<Option<PowerRef>> {
        self.apply_power_instance_from_card(None, target, kind, amount, applier)
    }

    /// [`Engine::apply_power_instance`] with a `cardSource`.
    pub(crate) fn apply_power_instance_from_card(
        &mut self,
        card: Option<CardId>,
        target: CreatureId,
        kind: PowerKind,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow<Option<PowerRef>> {
        self.apply_power_model(card, target, kind, amount, applier, None)
    }

    /// `PowerCmd.Apply(power, ...)` with `power` a `ClonePreservingMutability` of `source`
    /// (`Misery`): a new instance keeps the source's `AmountOnTurnStart` and
    /// `SkipNextDurationTick` (`AbstractModel.cs:173`, `MemberwiseClone`); its `_internalData` is
    /// fresh (`PowerModel.cs:590-595`) and `Applier` is set by `Apply` (`PowerCmd.cs:122`).
    pub(crate) fn apply_power_clone_from_card(
        &mut self,
        card: Option<CardId>,
        target: CreatureId,
        source: &Power,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        self.apply_power_model(card, target, source.kind, amount, applier, Some(source))
            .map(drop)
    }

    /// `PowerCmd.Apply` of a new power model of `kind`, or of a clone of `source`.
    fn apply_power_model(
        &mut self,
        card: Option<CardId>,
        target: CreatureId,
        kind: PowerKind,
        amount: Decimal,
        applier: Option<CreatureId>,
        source: Option<&Power>,
    ) -> Flow<Option<PowerRef>> {
        // `Apply(power, ...)` (a `source` clone) drops a zero amount first; `Apply<T>` only does so
        // for a new instance, so a zero still runs `ModifyAmount` on an existing one (its
        // `BeforePowerAmountChanged`: `UnsettlingLamp` latches onto a Malaise with X = 0).
        if self.hooked().is_ending()
            || (source.is_some() && amount == Decimal::ZERO)
            || !self.can_receive_powers(target)
        {
            return Ok(None);
        }
        // FindExistingInstanceForStacking (`PowerCmd.cs:168-178`).
        let existing = self
            .s
            .creature(target)
            .powers
            .iter()
            .find(|p| {
                p.kind == kind
                    && match kind.instance_type() {
                        PowerInstanceType::None => true,
                        PowerInstanceType::Instanced => false,
                        PowerInstanceType::InstancedPerApplier => p.applier == applier,
                    }
            })
            .map(|p| p.id);
        if let Some(power) = existing {
            let new_amount = self.modify_power_amount_by(card, target, power, amount, applier)?;
            return Ok((new_amount != 0).then_some(PowerRef {
                owner: target,
                id: power,
            }));
        }
        if amount == Decimal::ZERO {
            return Ok(None);
        }
        let query = PowerQuery {
            kind,
            target,
            applier,
            card,
        };
        self.before_power_amount_changed(query, amount)?;
        let (modified, given) = self.modify_given(query, amount);
        let (modified, received) = self.hooked().modify_power_amount_received(query, modified);
        kind.before_applied(self, target, modified, applier)?;
        if !self.can_receive_powers(target) {
            return Ok(None);
        }
        // ApplyInternal adds nothing for a zero amount (e.g. a debuff Artifact blocked).
        let added = if modified == Decimal::ZERO {
            None
        } else {
            let id = self.add_power(target, kind, modified, applier, source);
            self.record_power_received(target, kind, modified, applier);
            Some(id)
        };
        if let Some(given) = given {
            self.after_modifying_power_amount_given(query, &given)?;
        }
        self.after_modifying_power_amount_received(query, &received)?;
        if let Some(me) = added {
            kind.after_applied(self, me, applier)?;
            self.after_power_amount_changed(me, modified, applier)?;
        }
        Ok(added.filter(|me| {
            self.s
                .creature(me.owner)
                .powers
                .iter()
                .any(|p| p.id == me.id)
        }))
    }

    /// `PowerCmd.ModifyAmount` (`PowerCmd.cs:219-265`): `Amount += (int)offset` after the
    /// given/received modifiers, then remove the power if its amount says so
    /// (`PowerModel.ShouldRemoveDueToAmount`).
    pub(crate) fn modify_power_amount(
        &mut self,
        owner: CreatureId,
        power: PowerId,
        offset: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        self.modify_power_amount_by(None, owner, power, offset, applier)
            .map(drop)
    }

    /// [`Engine::modify_power_amount`] with a `cardSource` (`FranticEscape`).
    pub(crate) fn modify_power_amount_from_card(
        &mut self,
        card: Option<CardId>,
        owner: CreatureId,
        power: PowerId,
        offset: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        self.modify_power_amount_by(card, owner, power, offset, applier)
            .map(drop)
    }

    /// [`Engine::modify_power_amount`], returning the new amount as `ModifyAmount` does (0 if
    /// it did nothing).
    fn modify_power_amount_by(
        &mut self,
        card: Option<CardId>,
        owner: CreatureId,
        power: PowerId,
        offset: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow<i32> {
        // `owner.CombatState == null`: the owner left the combat (escaped, or a dead enemy removed).
        if self.hooked().is_ending() || !self.s.contains_creature(owner) {
            return Ok(0);
        }
        // The instance may already be off its owner (removed by a hook while its model still
        // runs); `ModifyAmount` still runs every hook and `SetAmount` on it (`PowerCmd.cs:219`).
        let Some(kind) = self.s.creature(owner).power_instance(power).map(|p| p.kind) else {
            return Ok(0);
        };
        let query = PowerQuery {
            kind,
            target: owner,
            applier,
            card,
        };
        self.before_power_amount_changed(query, offset)?;
        let (modified, given) = self.modify_given(query, offset);
        let (modified, received) = self.hooked().modify_power_amount_received(query, modified);
        self.record_power_received(owner, kind, modified, applier);
        let change = modified.to_int_truncating();
        let me = PowerRef { owner, id: power };
        let creature = self.s.creature_mut(owner);
        let attached = creature.powers.iter().any(|p| p.id == power);
        let p = creature
            .power_instance_mut(power)
            .expect("an instance is never dropped");
        // `power.Amount + (int)modifiedOffset` is unchecked C#: it wraps, then `SetAmount` clamps.
        p.amount = p
            .amount
            .wrapping_add(change)
            .clamp(-999_999_999, 999_999_999);
        let new_amount = p.amount;
        if attached {
            self.emit(Event::PowerChanged {
                creature: owner,
                kind,
                amount: new_amount,
            });
        }
        if let Some(given) = given {
            self.after_modifying_power_amount_given(query, &given)?;
        }
        self.after_modifying_power_amount_received(query, &received)?;
        if change != 0 {
            self.after_power_amount_changed(me, modified, applier)?;
        }
        // `ShouldRemoveDueToAmount`, then `Remove` (also on a detached instance: its
        // `AfterRemoved` runs again).
        let amount = self.s.power(owner, power).amount;
        let remove = if kind.allows_negative() {
            amount == 0
        } else {
            amount <= 0
        };
        if remove {
            self.remove_power(owner, power)?;
        }
        Ok(new_amount)
    }

    /// `ApplyInternal`: a new instance of `kind` (or a clone of `source`) on `target` with
    /// `(int)amount`. A debuff on a player skips its next duration tick (`PowerCmd.cs:148-150`).
    fn add_power(
        &mut self,
        target: CreatureId,
        kind: PowerKind,
        amount: Decimal,
        applier: Option<CreatureId>,
        source: Option<&Power>,
    ) -> PowerRef {
        let id = PowerId(self.s.next_power_id);
        self.s.next_power_id += 1;
        let amount = amount.to_int_truncating().clamp(-999_999_999, 999_999_999);
        let skip_next_duration_tick = source.is_some_and(|p| p.skip_next_duration_tick)
            || (self.s.creature(target).side == Side::Player
                && kind.power_type() == PowerType::Debuff);
        self.s.push_power(
            target,
            Power {
                id,
                kind,
                amount,
                amount_on_turn_start: source.map_or(0, |p| p.amount_on_turn_start),
                applier,
                skip_next_duration_tick,
            },
        );
        self.s.set_power_data(id, kind.init_internal_data());
        if kind == PowerKind::Minion {
            self.s.recount_enemies();
        }
        self.emit(Event::PowerChanged {
            creature: target,
            kind,
            amount,
        });
        PowerRef { owner: target, id }
    }

    /// `Hook.ModifyPowerAmountGiven`, only when the applier is still in combat
    /// (`PowerCmd.cs:127,234`); `None` for the modifiers when it was not run.
    fn modify_given(&self, query: PowerQuery, amount: Decimal) -> (Decimal, Option<Vec<Listener>>) {
        match query.applier {
            Some(applier) if self.s.contains_creature(applier) => {
                let (amount, modifiers) = self.hooked().modify_power_amount_given(query, amount);
                (amount, Some(modifiers))
            }
            _ => (amount, None),
        }
    }

    /// `PowerCmd.Decrement`: `ModifyAmount(-1)` with no applier.
    pub(crate) fn decrement_power(&mut self, me: PowerRef) -> Flow {
        self.modify_power_amount(me.owner, me.id, Decimal::from(-1), None)
    }

    /// `PowerCmd.Remove`: `RemoveInternal`, then the power's `AfterRemoved`. Not guarded. An
    /// instance already removed stays removed (`_powers.Remove` is a no-op) but its
    /// `AfterRemoved` runs again (`PowerCmd.cs:286-295`).
    pub(crate) fn remove_power(&mut self, owner: CreatureId, power: PowerId) -> Flow {
        let at = self
            .s
            .creature(owner)
            .powers
            .iter()
            .position(|p| p.id == power);
        let removed = if let Some(index) = at {
            let removed = self.s.take_power(owner, index);
            self.s.creature_mut(owner).removed_powers.push(removed);
            if removed.kind == PowerKind::Minion {
                self.s.recount_enemies();
            }
            removed
        } else if let Some(removed) = self.s.creature(owner).power_instance(power) {
            *removed
        } else {
            return Ok(());
        };
        removed.kind.model().after_removed(self, &removed, owner)
    }

    /// `Creature.CanReceivePowers`: in combat and `Hook.ShouldAllowHitting` allows it.
    /// `CombatHistory.PowerReceived` (the owner is the actor).
    fn record_power_received(
        &mut self,
        owner: CreatureId,
        kind: PowerKind,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) {
        let entry = HistoryKind::PowerReceived {
            kind,
            amount,
            applier,
        };
        self.record(owner, entry);
    }

    fn can_receive_powers(&self, target: CreatureId) -> bool {
        let creature = self.s.creature(target);
        // `CombatState != null`: an enemy in combat, or one `CreateCreature` made that has not
        // joined yet (`SurprisePower` powers the fat gremlin first); not one that left.
        let attached = creature.side != Side::Enemy
            || self.s.enemies.contains(&target)
            || (creature.is_alive() && !self.s.escaped.contains(&target));
        attached && self.hooked().should_allow_hitting(target)
    }
}
