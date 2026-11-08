//! `AttackCommand.Execute`, `CreatureCmd.Damage` and `CreatureCmd.Kill`.

use crate::combat::engine::{Engine, Event, Flow, ValueProps};
use crate::combat::history::HistoryKind;
use crate::combat::hooks::{DamageEvent, DamageQuery, HpLossPhase, HpLossQuery, PowerRef};
use crate::combat::state::{CardId, CreatureId, Creatures, PowerId, Side};
use crate::content::cards::Var;
use crate::content::monsters;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;
use crate::rng::RunRngType;
use smallvec::SmallVec;

/// `KillWithoutCheckingWinCondition` throws after this many prevented deaths in a row; the
/// engine panics there.
const MAX_DEATH_PREVENTIONS: u32 = 10;

/// The results of a damage or attack command, inline for the usual few.
pub type DamageResults = SmallVec<[DamageResult; 4]>;

/// `DamageResult` for one receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DamageResult {
    pub(crate) receiver: CreatureId,
    pub(crate) blocked: i32,
    /// HP actually lost.
    pub(crate) unblocked: i32,
    pub(crate) overkill: i32,
    pub(crate) was_target_killed: bool,
    pub(crate) was_block_broken: bool,
    pub(crate) was_fully_blocked: bool,
}

impl DamageResult {
    const fn empty(receiver: CreatureId) -> Self {
        Self {
            receiver,
            blocked: 0,
            unblocked: 0,
            overkill: 0,
            was_target_killed: false,
            was_block_broken: false,
            was_fully_blocked: false,
        }
    }
}

/// Damage per hit of an attack.
#[derive(Clone, Copy, Debug)]
pub enum AttackDamage {
    Fixed(Decimal),
    /// The card's `CalculatedDamageVar`, calculated per attack against its single target
    /// ([`CombatState::calculate`]).
    Calculated(CardId),
}

/// Who an attack can hit (`GetPossibleTargets`).
#[derive(Clone, Copy, Debug)]
pub enum AttackTargets {
    /// `Targeting(target)`.
    Single(CreatureId),
    /// A monster attack with no explicit target: every `PlayerCreatures` entry, never pets.
    PlayerCreatures,
    /// `TargetingAllOpponents` from a non-monster source: `GetOpponentsOf(attacker)`, re-read
    /// before every hit.
    AllOpponents,
    /// `TargetingRandomOpponents` (duplicates allowed): the possible targets of
    /// `AllOpponents`, one picked per hit on `Rng.CombatTargets`.
    RandomOpponents,
}

/// An `AttackCommand` as built by `DamageCmd.Attack(..).From*(..).Targeting(..)`.
#[derive(Clone, Copy, Debug)]
pub struct Attack {
    pub(crate) attacker: CreatureId,
    pub(crate) targets: AttackTargets,
    pub(crate) damage: AttackDamage,
    pub(crate) hits: u32,
    /// `DamageProps`: `Move` unless made unpowered.
    pub(crate) props: ValueProps,
    /// `ModelSource as CardModel`.
    pub(crate) card: Option<CardId>,
    /// The command's identity, set when it executes (hooks compare commands by reference).
    pub(crate) id: Option<AttackId>,
}

/// The identity of an executing `AttackCommand`, unique for the whole combat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttackId(pub u32);

impl Attack {
    /// A single-hit powered attack.
    pub(crate) const fn new(
        attacker: CreatureId,
        targets: AttackTargets,
        damage: AttackDamage,
        card: Option<CardId>,
    ) -> Self {
        Self {
            attacker,
            targets,
            damage,
            hits: 1,
            props: ValueProps::MOVE,
            card,
            id: None,
        }
    }

    /// `WithHitCount(hits)`.
    #[must_use]
    pub(crate) const fn with_hits(mut self, hits: u32) -> Self {
        self.hits = hits;
        self
    }
}

impl Engine<'_> {
    /// `AttackCommand.Execute` (`AttackCommand.cs:526-674`). Returns every hit's results
    /// (`command.Results`).
    pub(crate) fn attack(&mut self, attack: &Attack) -> Flow<DamageResults> {
        let mut results = DamageResults::new();
        if self.hooked().is_over_or_ending() || self.s.creature(attack.attacker).is_dead() {
            return Ok(results);
        }
        let attack = &Attack {
            id: Some(AttackId(self.s.next_attack_id)),
            ..*attack
        };
        self.s.next_attack_id += 1;
        self.before_attack(attack)?;
        // Hook.ModifyAttackHitCount: no in-scope listener.
        let (mut one, mut list) = ([CreatureId(0)], Creatures::new());
        if self.records_events() {
            let valid = self.valid_targets(attack, &mut one, &mut list);
            let damage = self.hit_amount(attack, valid);
            self.emit(Event::Attack {
                attacker: attack.attacker,
                damage,
                hits: attack.hits,
            });
        }
        for _ in 0..attack.hits {
            if self.s.creature(attack.attacker).is_dead() {
                break;
            }
            let mut valid = self.valid_targets(attack, &mut one, &mut list);
            if valid.is_empty() {
                break;
            }
            let mut picked = [CreatureId(0)];
            if matches!(attack.targets, AttackTargets::RandomOpponents) {
                let target = self.random_target(valid);
                valid = match target {
                    Some(target) => {
                        picked[0] = target;
                        &picked
                    }
                    None => &[],
                };
            }
            let amount = self.hit_amount(attack, valid);
            self.damage_into(
                valid,
                amount,
                attack.props,
                Some(attack.attacker),
                attack.card,
                &mut results,
            )?;
        }
        self.record(attack.attacker, HistoryKind::CreatureAttacked);
        self.after_attack(attack, &results)?;
        Ok(results)
    }

    /// Whether a kill of `target` can trigger Fatal: every power of the target allows it
    /// (`target.Powers.All(p => p.ShouldOwnerDeathTriggerFatal())`). Read before the attack.
    pub(crate) fn target_allows_fatal(&self, target: CreatureId) -> bool {
        self.s.creature(target).powers.iter().all(|p| {
            let me = PowerRef {
                owner: target,
                id: p.id,
            };
            p.kind.should_owner_death_trigger_fatal(self.s, me)
        })
    }

    /// `results.Any(r => r.WasTargetKilled)`: whether the attack killed anything.
    pub(crate) fn fatal_triggered(results: &[DamageResult]) -> bool {
        results.iter().any(|r| r.was_target_killed)
    }

    /// The living creatures the next hit of `attack` can land on: a single target or the
    /// player in `one`, a side in `list` (filled in place: a returned list is copied right
    /// after its stores, which stalls).
    fn valid_targets<'b>(
        &self,
        attack: &Attack,
        one: &'b mut [CreatureId; 1],
        list: &'b mut Creatures,
    ) -> &'b [CreatureId] {
        let alive = |c: CreatureId| self.s.creature(c).is_alive();
        let single = match attack.targets {
            AttackTargets::Single(target) => Some(target),
            // `PlayerCreatures`: the only player is `player.creature`, while it is an ally.
            AttackTargets::PlayerCreatures => {
                let me = self.s.player.creature;
                let is_ally = self.s.allies.contains(&me);
                debug_assert!(
                    self.s.player_creatures().iter().eq(is_ally.then_some(&me)),
                    "the player creature is the only player"
                );
                if !is_ally {
                    return &[];
                }
                Some(me)
            }
            AttackTargets::AllOpponents | AttackTargets::RandomOpponents => None,
        };
        if let Some(target) = single {
            if !alive(target) {
                return &[];
            }
            one[0] = target;
            return one;
        }
        list.clear();
        list.extend_from_slice(match self.s.creature(attack.attacker).side {
            Side::Player => &self.s.enemies,
            Side::Enemy => &self.s.allies,
        });
        list.retain(|c| alive(*c));
        list
    }

    /// The unmodified damage of one hit on `valid` targets.
    fn hit_amount(&self, attack: &Attack, valid: &[CreatureId]) -> Decimal {
        match attack.damage {
            AttackDamage::Fixed(amount) => amount,
            AttackDamage::Calculated(card) => {
                let single_target = (valid.len() == 1).then(|| valid[0]);
                self.s
                    .calculate(card, Var::CalculatedDamage, single_target)
                    .expect("a calculated attack's card has a CalculatedDamage var")
            }
        }
    }

    /// Random target pick for randomly targeted attacks (`AttackCommand.cs:617`).
    pub(crate) fn random_target(&mut self, valid: &[CreatureId]) -> Option<CreatureId> {
        self.s.rng[RunRngType::CombatTargets]
            .next_item(valid)
            .copied()
    }

    /// `CreatureCmd.Damage` (`CreatureCmd.cs:258-433`): per living target, `ModifyDamage`,
    /// `BeforeDamageReceived`, block (the pet owner's), `ModifyHpLost(BeforeOsty)`, the Osty
    /// redirect, `ModifyHpLost(AfterOsty)`, HP loss; then per result `AfterBlockBroken`,
    /// `AfterCurrentHpChanged`, `AfterDamageGiven` and `AfterDamageReceived` (or a kill).
    pub(crate) fn damage(
        &mut self,
        targets: &[CreatureId],
        amount: Decimal,
        props: ValueProps,
        dealer: Option<CreatureId>,
        card: Option<CardId>,
    ) -> Flow<DamageResults> {
        let mut results = DamageResults::new();
        self.damage_into(targets, amount, props, dealer, card, &mut results)?;
        Ok(results)
    }

    /// [`Engine::damage`], appending the results to `out` (built in place, not copied).
    pub(crate) fn damage_into(
        &mut self,
        targets: &[CreatureId],
        amount: Decimal,
        props: ValueProps,
        dealer: Option<CreatureId>,
        card: Option<CardId>,
        out: &mut DamageResults,
    ) -> Flow {
        if dealer.is_some_and(|d| self.s.creature(d).is_dead()) {
            out.extend(targets.iter().map(|&t| DamageResult::empty(t)));
            return Ok(());
        }
        let start = out.len();
        for &original_target in targets {
            if self.s.creature(original_target).is_dead() {
                continue;
            }
            self.damage_target(original_target, amount, props, dealer, card, out)?;
        }

        let mut killed = Creatures::new();
        for &result in &out[start..] {
            let target = result.receiver;
            if result.was_block_broken {
                self.after_block_broken(target, dealer)?;
            }
            if result.unblocked > 0 {
                self.after_current_hp_changed(target, Decimal::from(-result.unblocked))?;
            }
            let hit = DamageEvent {
                target,
                result,
                props,
                dealer,
                card,
            };
            self.after_damage_given(hit)?;
            if result.was_target_killed && self.s.creature(target).is_dead() {
                killed.push(target);
            } else {
                self.after_damage_received(hit)?;
            }
        }
        self.kill(&killed)?;
        Ok(())
    }

    /// One target of `CreatureCmd.Damage` (`CreatureCmd.cs:280-312`): its results, one, or two
    /// when the unblocked damage was redirected (the redirect target's, then the original
    /// target's).
    fn damage_target(
        &mut self,
        original_target: CreatureId,
        amount: Decimal,
        props: ValueProps,
        dealer: Option<CreatureId>,
        card: Option<CardId>,
        out: &mut DamageResults,
    ) -> Flow {
        let query = DamageQuery {
            target: Some(original_target),
            dealer,
            props,
            card,
        };
        let (modified, modifiers) = self.hooked().modify_damage(query, amount);
        self.after_modifying_damage_amount(card, &modifiers)?;
        let hp_query = |target| HpLossQuery {
            target,
            props,
            dealer,
            card,
        };
        self.before_damage_received(hp_query(original_target), modified)?;

        // Block is the pet owner's (`CreatureCmd.cs:285`).
        let block_holder = self
            .s
            .creature(original_target)
            .pet_owner
            .unwrap_or(original_target);
        let blocked = self.damage_block_internal(block_holder, modified, props);
        let (unblocked, modifiers) = self.hooked().modify_hp_lost(
            hp_query(original_target),
            (modified - blocked).max(Decimal::ZERO),
            HpLossPhase::BeforeOsty,
        );
        self.after_modifying_hp_lost(HpLossPhase::BeforeOsty, &modifiers)?;
        let receiver =
            self.hooked()
                .modify_unblocked_damage_target(original_target, unblocked, props, dealer);
        let (unblocked, modifiers) =
            self.hooked()
                .modify_hp_lost(hp_query(receiver), unblocked, HpLossPhase::AfterOsty);
        self.after_modifying_hp_lost(HpLossPhase::AfterOsty, &modifiers)?;
        let hit = self.lose_hp_internal(receiver, unblocked);

        let target_block = self.s.creature(original_target).block;
        let was_block_broken = target_block <= 0 && blocked > Decimal::ZERO;
        let was_fully_blocked = !props.has(ValueProps::UNBLOCKABLE)
            && (blocked > Decimal::ZERO || target_block > 0)
            && unblocked.to_int_truncating() == 0;
        let blocked = blocked.to_int_truncating();
        if receiver == original_target {
            let result = DamageResult {
                blocked,
                was_block_broken,
                was_fully_blocked,
                ..hit
            };
            out.push(result);
            self.record_damage_received(result, props, dealer, card);
        } else {
            // Overkill on the redirect target goes back to the original target, through
            // `ModifyHpLost(AfterOsty)` again (`CreatureCmd.cs:302-311`).
            let overkill = Decimal::from(hit.overkill);
            out.push(hit);
            let (rest_amount, modifiers) = self.hooked().modify_hp_lost(
                hp_query(original_target),
                overkill,
                HpLossPhase::AfterOsty,
            );
            self.after_modifying_hp_lost(HpLossPhase::AfterOsty, &modifiers)?;
            let rest = if rest_amount > Decimal::ZERO {
                self.lose_hp_internal(original_target, rest_amount)
            } else {
                DamageResult::empty(original_target)
            };
            let rest = DamageResult {
                blocked,
                was_block_broken,
                was_fully_blocked,
                ..rest
            };
            out.push(rest);
            self.record_damage_received(hit, props, dealer, card);
            self.record_damage_received(rest, props, dealer, card);
        }
        Ok(())
    }

    /// `CombatHistory.DamageReceived` for one result, every result including fully blocked
    /// ones (`CreatureCmd.cs:316-320`), and its event. Takes the result by value: read back
    /// from the list right after it was pushed, it would stall (store forwarding).
    fn record_damage_received(
        &mut self,
        result: DamageResult,
        props: ValueProps,
        dealer: Option<CreatureId>,
        card: Option<CardId>,
    ) {
        if !self.hooked().is_over_or_ending() {
            self.record(
                result.receiver,
                HistoryKind::DamageReceived {
                    result,
                    props,
                    dealer,
                    card_source: card,
                },
            );
        }
        self.emit(Event::Damage {
            dealer,
            receiver: result.receiver,
            blocked: result.blocked,
            hp_lost: result.unblocked,
        });
    }

    /// `Creature.DamageBlockInternal`: `min(Block, amount)` (0 if unblockable), and
    /// `Block -= (int)blocked`.
    fn damage_block_internal(
        &mut self,
        creature: CreatureId,
        amount: Decimal,
        props: ValueProps,
    ) -> Decimal {
        let creature = self.s.creature_mut(creature);
        let blocked = if props.has(ValueProps::UNBLOCKABLE) {
            Decimal::ZERO
        } else {
            Decimal::from(creature.block).min(amount)
        };
        creature.block -= blocked.to_int_truncating();
        blocked
    }

    /// `Creature.LoseHpInternal` (`Creature.cs:446`). "Killed" is decided on the decimal
    /// amount, before truncation.
    #[inline]
    pub(crate) fn lose_hp_internal(&mut self, id: CreatureId, amount: Decimal) -> DamageResult {
        let creature = self.s.creature_mut(id);
        let hp_before = creature.hp;
        let was_target_killed = hp_before > 0 && amount >= Decimal::from(hp_before);
        let lost = amount
            .clamp(Decimal::ZERO, Decimal::from(999_999_999))
            .to_int_truncating();
        creature.hp = (hp_before - lost).max(0);
        let hp = creature.hp;
        if hp == 0 && hp_before > 0 && creature.side == Side::Enemy {
            self.s.recount_enemies();
        }
        DamageResult {
            receiver: id,
            blocked: 0,
            unblocked: hp_before - hp,
            overkill: if was_target_killed {
                (lost - hp_before).max(0)
            } else {
                0
            },
            was_target_killed,
            was_block_broken: false,
            was_fully_blocked: false,
        }
    }

    /// `CreatureCmd.Kill(creatures)` (`CreatureCmd.cs:450-507`): kills each, then flags a
    /// pending loss when every player is dead. Win/loss itself is only evaluated at
    /// `CheckWinCondition` safe points.
    pub(crate) fn kill(&mut self, creatures: &[CreatureId]) -> Flow {
        self.kill_with(creatures, false)
    }

    /// `CreatureCmd.Kill(creatures, force: true)`: no death prevention (`SandpitPower`).
    pub(crate) fn kill_forced(&mut self, creatures: &[CreatureId]) -> Flow {
        self.kill_with(creatures, true)
    }

    fn kill_with(&mut self, creatures: &[CreatureId], force: bool) -> Flow {
        if creatures.is_empty() {
            return Ok(());
        }
        let has_player = creatures.iter().any(|&c| self.s.creature(c).is_player());
        for &creature in creatures {
            if !self.has_left_combat(creature) {
                self.kill_without_checking_win_condition(creature, force, 0)?;
            }
        }
        let all_players_dead = self
            .s
            .player_creatures()
            .iter()
            .all(|&p| self.s.creature(p).is_dead());
        if has_player && all_players_dead && self.s.in_progress {
            // CombatManager.LoseCombat.
            self.s.pending_loss = true;
        }
        // else: a dead player on the player side ends their turn (multiplayer only).
        Ok(())
    }

    /// `KillWithoutCheckingWinCondition`'s first guard (`CreatureCmd.cs:517`): a creature that
    /// has left the combat (`RemoveCreature` unattaches it: a removed dead enemy or pet, an
    /// escaped enemy) is not killed again; the player always is. A death the guard lets through
    /// is prevented in place, so the recursion never needs it.
    fn has_left_combat(&self, id: CreatureId) -> bool {
        !self.s.contains_creature(id) && !self.s.creature(id).is_player()
    }

    /// `KillWithoutCheckingWinCondition` (`CreatureCmd.cs:514-602`), past its first guard
    /// ([`Engine::has_left_combat`]).
    fn kill_without_checking_win_condition(
        &mut self,
        id: CreatureId,
        force: bool,
        recursion: u32,
    ) -> Flow {
        let hp = self.s.creature(id).hp;
        if hp > 0 {
            self.lose_hp_internal(id, Decimal::from(hp));
            self.after_current_hp_changed(id, Decimal::from(-hp))?;
        }
        self.before_death(id)?;
        let preventer = if force || self.s.creature(id).max_hp <= 0 {
            None
        } else {
            self.hooked().death_preventer(id)
        };
        if let Some(preventer) = preventer {
            assert!(
                recursion < MAX_DEATH_PREVENTIONS,
                "something keeps preventing a creature from being killed"
            );
            self.after_death(id, true)?;
            self.after_preventing_death(preventer, id)?;
            if self.s.creature(id).is_dead() {
                self.kill_without_checking_win_condition(id, force, recursion + 1)?;
            }
            return Ok(());
        }
        self.emit(Event::Died(id));
        let remove_from_combat = self.hooked().should_creature_be_removed_after_death(id);
        self.after_death(id, false)?;
        let side = self.s.creature(id).side;
        let teammates: Vec<CreatureId> = self
            .s
            .creatures_on(side)
            .into_iter()
            .filter(|&t| self.s.creature(t).is_alive())
            .collect();
        if remove_from_combat
            && self.s.creature(id).side == Side::Enemy
            && self.s.enemies.contains(&id)
        {
            // CombatManager.RemoveCreature: BeforeRemovedFromRoom, then the state machine is
            // dropped (the dead monster never rolls again).
            monsters::before_removed_from_room(self, id);
            let performing = self
                .s
                .creature(id)
                .monster
                .as_ref()
                .is_some_and(|m| m.is_performing_move);
            if !performing {
                self.s.remove_enemy(id);
            }
        }
        let is_primary_enemy = self.s.creature(id).is_primary_enemy();
        // RemoveAllPowersAfterDeath: keep the powers that survive their owner's death or that a
        // listener keeps (`Creature.cs:673`), then AfterRemoved each removed one.
        let hooked = self.hooked();
        let gone: Vec<PowerId> = hooked
            .s
            .creature(id)
            .powers
            .iter()
            .filter(|p| {
                p.kind.should_be_removed_after_owner_death()
                    && hooked.should_power_be_removed_on_death(PowerRef {
                        owner: id,
                        id: p.id,
                    })
            })
            .map(|p| p.id)
            .collect();
        let removed = self.s.take_powers_where(id, |p| gone.contains(&p.id));
        self.s
            .creature_mut(id)
            .removed_powers
            .extend(removed.iter().copied());
        if removed.iter().any(|p| p.kind == PowerKind::Minion) {
            self.s.recount_enemies();
        }
        for power in removed {
            power.kind.model().after_removed(self, &power, id)?;
        }
        // The last primary enemy takes its living secondary teammates (Minions) with it.
        if side == Side::Enemy {
            if is_primary_enemy
                && !teammates.is_empty()
                && teammates
                    .iter()
                    .all(|&t| self.s.creature(t).is_secondary_enemy())
            {
                self.kill(&teammates)?;
            }
            return Ok(());
        }
        // A dead player's Osty dies too, then the player's relics, potions and cards stop
        // listening (DeactivateHooks). HandlePlayerDeath does nothing once every player is dead.
        if self.s.creature(id).is_player() {
            // OrbQueue.Clear (`CreatureCmd.cs:578`): no orbs and no slots.
            self.s.player.orbs.clear();
            self.s.player.orb_slots = 0;
            if let Some(osty) = self.s.osty().filter(|_| self.s.is_osty_alive()) {
                self.kill_with(&[osty], force)?;
            }
            self.s.set_hooks_active(false);
        }
        Ok(())
    }
}
