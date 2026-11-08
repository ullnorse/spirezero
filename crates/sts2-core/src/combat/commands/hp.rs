//! Block and HP/max-HP changes (`CreatureCmd.GainBlock`, `LoseBlock`, `Heal`, `SetMaxHp`,
//! `GainMaxHp`).

use crate::combat::card_play::CardPlay;
use crate::combat::engine::{Engine, Event, Flow, ValueProps};
use crate::combat::history::HistoryKind;
use crate::combat::hooks::BlockQuery;
use crate::combat::state::CreatureId;
use crate::decimal::Decimal;

impl Engine<'_> {
    /// `CreatureCmd.GainBlock` (`CreatureCmd.cs:665-698`). Returns the modified amount. `play`
    /// is the `CardPlay` that caused it (`None` if not directly a card play); a positive gain is
    /// logged as `BlockGainedEntry`.
    pub(crate) fn gain_block(
        &mut self,
        id: CreatureId,
        amount: Decimal,
        props: ValueProps,
        play: Option<&CardPlay>,
    ) -> Flow<Decimal> {
        if self.hooked().is_over_or_ending() || self.s.creature(id).is_dead() {
            return Ok(Decimal::ZERO);
        }
        let card = play.map(|p| p.card);
        let query = BlockQuery {
            target: id,
            props,
            card,
            play: play.copied(),
        };
        self.before_block_gained(query, amount)?;
        let (modified, modifiers) = self.hooked().modify_block(query, amount);
        let modified = modified.max(Decimal::ZERO);
        self.after_modifying_block_amount(modified, card, play, &modifiers)?;
        if modified > Decimal::ZERO {
            let creature = self.s.creature_mut(id);
            // GainBlockInternal: truncating, capped at 999999999.
            creature.block = (Decimal::from(creature.block) + modified)
                .min(Decimal::from(999_999_999))
                .to_int_truncating();
            self.emit(Event::BlockGained {
                creature: id,
                amount: modified.to_int_truncating(),
            });
            self.record(
                id,
                HistoryKind::BlockGained {
                    amount: modified.to_int_truncating(),
                    props,
                    play: play.copied(),
                },
            );
        }
        self.after_block_gained(query, modified)?;
        Ok(modified)
    }

    /// `CreatureCmd.LoseBlock(target, amount, remover)` (`CreatureCmd.cs:710-722`): nothing once
    /// combat is over or ending, for a dead target or a non-positive amount; else
    /// `LoseBlockInternal` (`max(Block - amount, 0)`, truncated), then `AfterBlockBroken` if
    /// that took the last of the target's block.
    pub(crate) fn lose_block(
        &mut self,
        target: CreatureId,
        amount: Decimal,
        remover: Option<CreatureId>,
    ) -> Flow {
        if self.hooked().is_over_or_ending()
            || self.s.creature(target).is_dead()
            || amount <= Decimal::ZERO
        {
            return Ok(());
        }
        let creature = self.s.creature_mut(target);
        let before = creature.block;
        creature.block = (Decimal::from(before) - amount)
            .max(Decimal::ZERO)
            .to_int_truncating();
        if before > 0 && creature.block <= 0 {
            self.after_block_broken(target, remover)?;
        }
        Ok(())
    }

    /// `CreatureCmd.Heal` (`CreatureCmd.cs:735-807`): `HealInternal` clamps to max HP and
    /// reactivates a revived player's hooks; then `AfterCurrentHpChanged(amount)` for a
    /// positive amount.
    pub(crate) fn heal(&mut self, id: CreatureId, amount: Decimal) -> Flow {
        if self.hooked().is_ending() && !self.s.creature(id).is_player() {
            return Ok(());
        }
        let creature = self.s.creature_mut(id);
        let was_dead = creature.is_dead();
        creature.hp = (Decimal::from(creature.hp) + amount)
            .min(Decimal::from(creature.max_hp))
            .to_int_truncating();
        if was_dead && creature.is_alive() {
            if creature.is_player() {
                self.s.set_hooks_active(true);
            } else {
                self.s.recount_enemies();
            }
        }
        if amount > Decimal::ZERO {
            self.after_current_hp_changed(id, amount)?;
        }
        Ok(())
    }

    /// `CreatureCmd.SetMaxHp`: returns the change in max HP. Current HP is clamped to it.
    pub(crate) fn set_max_hp(&mut self, id: CreatureId, amount: Decimal) -> Flow<i32> {
        let creature = self.s.creature_mut(id);
        let old = creature.max_hp;
        creature.max_hp = amount
            .max(Decimal::ZERO)
            .to_int_truncating()
            .min(999_999_999);
        let was_alive = creature.is_alive();
        creature.hp = creature.hp.min(creature.max_hp);
        let new = creature.max_hp;
        if was_alive && creature.is_dead() {
            self.s.recount_enemies();
        }
        if new <= 0 {
            self.kill(&[id])?;
        }
        Ok(new - old)
    }

    /// `CreatureCmd.GainMaxHp`: raise max HP, then heal by the change.
    pub(crate) fn gain_max_hp(&mut self, id: CreatureId, amount: Decimal) -> Flow {
        let delta = self.set_max_hp(id, Decimal::from(self.s.creature(id).max_hp) + amount)?;
        self.heal(id, Decimal::from(delta))
    }
}
