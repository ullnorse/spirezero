//! The turn loop (`CombatManager.StartCombatInternal`, `StartTurn`, end-of-turn phases, enemy
//! turn) and the `CheckWinCondition` safe points.

use super::commands::MAX_HAND_SIZE;
use super::commands::Position;
use super::engine::{Engine, Event, Flow};
use super::state::{CardId, CombatResult, CreatureId, Creatures, PileType, Side, TurnPhase};
use crate::content::cards::CardKeyword;
use crate::content::{cards, monsters};
use crate::decimal::Decimal;

/// `ModifyHandDraw` base value.
const HAND_DRAW: i32 = 5;

impl Engine<'_> {
    /// `CombatRoom.StartCombat` after `SetUpCombat` (`AfterRoomEntered`), then combat start
    /// (`CombatManager.cs:576-626`) up to the first player play phase.
    pub(crate) fn start_combat(&mut self) -> Flow {
        self.after_room_entered()?;
        // AfterCreatureAdded for each creature (allies, then enemies): an enemy's
        // AfterAddedToRoom, then its first move roll. A roll from a move state is a no-op
        // until a move has been performed; a branch state transitions right away.
        for id in self.s.enemies.clone() {
            monsters::after_added_to_room(self, id)?;
            monsters::roll_move(self, id);
        }
        self.s.in_progress = true;
        self.s.starting = false;
        self.before_combat_start()?;
        self.start_turn(&[])?;
        self.end_turn_if_ready()
    }

    /// `EndPlayerTurnAction` and everything up to the next player play phase or combat end.
    pub(crate) fn end_player_turn(&mut self) -> Flow {
        loop {
            self.end_player_turn_to_enemy_turn_end(&[])?;
            // The player's next turn (a no-op once combat ended).
            self.start_turn(&[])?;
            if !self.ends_turn_now() {
                return Ok(());
            }
        }
    }

    /// [`Engine::end_player_turn`] up to the start of the player's next turn: the end of the
    /// player's turn, then the enemy turn unless the player takes an extra turn. The monsters
    /// in `idle` skip their move (a preview leaves out the moves the player cannot see).
    pub(crate) fn end_player_turn_to_enemy_turn_end(&mut self, idle: &[CreatureId]) -> Flow {
        if self.end_player_turn_phase_one()? || !self.s.in_progress {
            return Ok(());
        }
        self.end_player_turn_phase_two()?;
        self.switch_from_player_to_enemy_side()?;
        if self.s.player.extra_turn {
            // The turn loop starts the player's extra turn: no enemy turn in between.
            return Ok(());
        }
        // Enemy turn: StartTurn runs ExecuteEnemyTurn and EndEnemyTurn, which switches back.
        self.start_turn(idle)
    }

    /// `PlayerCmd.EndTurn(player, canBackOut: false)` (`PlayerCmd.cs:286`): the player is ready
    /// to end the turn (`SetReadyToEndTurn`); the turn ends once the running action finished.
    pub(crate) const fn end_turn(&mut self) {
        self.s.player.ready_to_end_turn = true;
    }

    /// The end of a player action: a player who became ready to end the turn during it ends
    /// it now (`AfterAllPlayersReadyToEndTurn` waits for the running action,
    /// `CombatManager.cs:660-672`).
    pub(crate) fn end_turn_if_ready(&mut self) -> Flow {
        if self.ends_turn_now() {
            self.end_player_turn()?;
        }
        Ok(())
    }

    /// Whether the player's turn ends before they act again: they became ready to end it
    /// during the running action, or during the turn start after `StartTurn` reset the ready
    /// set (`CombatManager.cs:729`), e.g. a `VoidForm` auto-played by `MayhemPower` in the
    /// auto-pre-play phase. The turn loop awaits the end-turn signal right after `StartTurn`
    /// (`CombatManager.cs:610-618,656`), so a signal set during the turn start ends the turn
    /// before the player gets control.
    fn ends_turn_now(&self) -> bool {
        self.s.player.ready_to_end_turn
            && self.s.in_progress
            && self.s.side == Side::Player
            && self.s.player.phase == TurnPhase::Play
    }

    /// `StartTurn` (`CombatManager.cs:688-851`) for the current side; on the enemy side the
    /// monsters in `idle` skip their move ([`Engine::end_player_turn_to_enemy_turn_end`]).
    fn start_turn(&mut self, idle: &[CreatureId]) -> Flow {
        if !self.s.in_progress {
            return Ok(());
        }
        let side = self.s.side;
        let round = self.s.round;
        self.emit(Event::TurnStarted { side, round });
        // An extra turn starts for the players taking it only (not Osty), without the
        // enemies' `PrepareForNextTurn` and `RollMove` (`CombatManager.cs:699-707,758`).
        let extra_turn = side == Side::Player && self.s.player.extra_turn;
        let creatures = if extra_turn {
            self.s.player_creatures()
        } else {
            self.s.creatures_on(side)
        };
        if side == Side::Player {
            self.s.player.phase = TurnPhase::None;
        }
        // Creature.BeforeTurnStart.
        for &id in &creatures {
            for power in &mut self.s.creature_mut(id).powers {
                power.amount_on_turn_start = power.amount;
            }
        }
        self.before_side_turn_start(side, &creatures)?;
        if side == Side::Player {
            self.s.player.phase = TurnPhase::Start;
            self.s.player.ready_to_end_turn = false;
            if !extra_turn {
                let enemies = Creatures::from_slice(&self.s.enemies);
                for &id in &enemies {
                    monsters::roll_move(self, id);
                }
            }
        }
        for &id in &creatures {
            self.after_turn_start(id)?;
        }
        for &id in &creatures {
            self.after_block_cleared(id)?;
        }
        if side == Side::Player {
            self.setup_player_turn()?;
        }
        self.after_side_turn_start(side, &creatures)?;
        if side == Side::Player {
            self.orbs_after_turn_start()?;
            // RunAutoPrePlayPhase (`CombatManager.cs:859-866`), for a living player.
            if !self.player_dead() {
                self.s.player.phase = TurnPhase::AutoPrePlay;
                self.check_for_empty_hand()?;
                self.after_auto_pre_play_phase_entered()?;
                self.s.player.phase = TurnPhase::Play;
            }
            self.check_win_condition()?;
            Ok(())
        } else {
            if self.check_win_condition()? {
                return Ok(());
            }
            self.execute_enemy_turn(idle)
        }
    }

    /// `Creature.AfterTurnStart` (`Creature.cs:686-733`): `ClearBlock`, skipped for a player on
    /// turn 1; a `ShouldClearBlock` veto keeps the block and calls the preventer's
    /// `AfterPreventingBlockClear`.
    fn after_turn_start(&mut self, id: CreatureId) -> Flow {
        if self.s.creature(id).is_player() && self.s.player.turn_number == 1 {
            return Ok(());
        }
        match self.hooked().block_clear_preventer(id) {
            None => {
                self.s.creature_mut(id).block = 0;
                Ok(())
            }
            Some(preventer) => self.after_preventing_block_clear(preventer, id),
        }
    }

    /// `SetupPlayerTurn` (`CombatManager.cs:877-924`).
    fn setup_player_turn(&mut self) -> Flow {
        if self.player_dead() {
            return Ok(());
        }
        let max_energy = self.hooked().max_energy();
        if self.hooked().should_player_reset_energy() {
            self.s.player.energy = max_energy;
        } else {
            self.s.player.energy += max_energy;
        }
        self.after_energy_reset()?;
        self.before_hand_draw()?;
        let (mut hand_draw, modifiers) = self.hooked().modify_hand_draw(Decimal::from(HAND_DRAW));
        self.after_modifying_hand_draw(&modifiers)?;
        if self.s.player.turn_number == 1 {
            // Each card whose enchantment `ShouldStartAtBottomOfDrawPile`, in draw-pile order,
            // moves to the bottom (`MoveToBottomInternal`); then each other Innate card moves to
            // the top (`MoveToTopInternal`), so the last one ends up first; the hand draw is at
            // least that Innate count, at most the hand size.
            let bottom: Vec<CardId> = self
                .s
                .player
                .draw
                .iter()
                .copied()
                .filter(|&c| {
                    self.s
                        .card(c)
                        .enchantment
                        .is_some_and(|e| e.kind.should_start_at_bottom_of_draw_pile())
                })
                .collect();
            for &card in &bottom {
                let draw = &mut self.s.player.draw;
                draw.retain(|&c| c != card);
                draw.push(card);
            }
            let hooked = self.hooked();
            let innate: Vec<CardId> = hooked
                .s
                .player
                .draw
                .iter()
                .copied()
                .filter(|&c| hooked.has_keyword(c, CardKeyword::Innate) && !bottom.contains(&c))
                .collect();
            for &card in &innate {
                let draw = &mut self.s.player.draw;
                draw.retain(|&c| c != card);
                draw.insert(0, card);
            }
            let count = i32::try_from(innate.len()).expect("pile size fits i32");
            hand_draw = hand_draw.max(Decimal::from(count)).min(Decimal::from(
                i32::try_from(MAX_HAND_SIZE).expect("the hand size fits i32"),
            ));
        }
        self.draw(hand_draw, true)?;
        self.after_player_turn_start()
    }

    /// `CheckForEmptyHand` (`CombatManager.cs:1171-1177`): `AfterHandEmptied` when combat is in
    /// progress, no card or potion effect is running and the hand is empty.
    pub(crate) fn check_for_empty_hand(&mut self) -> Flow {
        if self.s.in_progress && self.s.player.effect_depth == 0 && self.s.player.hand.is_empty() {
            self.after_hand_emptied()?;
        }
        Ok(())
    }

    /// End-turn phase one (`CombatManager.cs:1515-1590`). Returns whether combat ended.
    fn end_player_turn_phase_one(&mut self) -> Flow<bool> {
        self.s.player.phase = TurnPhase::AutoPostPlay;
        self.after_auto_post_play_phase_entered()?;
        self.s.player.phase = TurnPhase::End;
        let players = self.s.player_creatures();
        self.before_side_turn_end(Side::Player, &players)?;
        if self.check_win_condition()? {
            return Ok(true);
        }
        self.do_turn_end()?;
        if self.check_win_condition()? {
            return Ok(true);
        }
        self.before_flush()?;
        self.check_win_condition()
    }

    /// `DoTurnEnd` (`CombatManager.cs:1599-1690`): the orbs' end-of-turn passives
    /// (`OrbQueue.BeforeTurnEnd`); then, unless combat is ending, exhaust the
    /// Ethereal hand cards, then resolve each card with a turn-end-in-hand effect in hand
    /// order: to the Play pile, `OnTurnEndInHand`, then exhausted if Ethereal or to the
    /// discard bottom. The non-interactive game resolves the cards one after another.
    fn do_turn_end(&mut self) -> Flow {
        self.orbs_before_turn_end()?;
        if !self.s.in_progress || self.hooked().is_ending() {
            return Ok(());
        }
        let mut turn_end_cards = Vec::new();
        let mut ethereal_cards = Vec::new();
        let hooked = self.hooked();
        for &card in &hooked.s.player.hand {
            let c = hooked.s.card(card);
            if c.kind.has_turn_end_in_hand_effect() {
                turn_end_cards.push(card);
            } else if hooked.has_keyword(card, CardKeyword::Ethereal)
                && hooked.should_ethereal_trigger(card)
            {
                ethereal_cards.push(card);
            }
        }
        for card in ethereal_cards {
            self.exhaust(card, true)?;
        }
        for card in turn_end_cards {
            self.add_to_pile(card, PileType::Play, Position::Bottom)?;
            cards::on_turn_end_in_hand(self, card)?;
            if self.hooked().has_keyword(card, CardKeyword::Ethereal) {
                self.exhaust(card, true)?;
            } else {
                self.add_to_pile(card, PileType::Discard, Position::Bottom)?;
            }
        }
        Ok(())
    }

    /// End-turn phase two (`CombatManager.cs:1746-1780`).
    fn end_player_turn_phase_two(&mut self) -> Flow {
        self.flush_hand()?;
        let players = self.s.player_creatures();
        self.after_side_turn_end(Side::Player, &players)
    }

    /// `ExecuteEnemyTurn` (`CombatManager.cs:1402-1435`) then `EndEnemyTurn`; the monsters in
    /// `idle` do not move.
    fn execute_enemy_turn(&mut self, idle: &[CreatureId]) -> Flow {
        let enemies = Creatures::from_slice(&self.s.enemies);
        for &id in &enemies {
            if !self.s.contains_creature(id) {
                continue;
            }
            // Creature.TakeTurn.
            let spawned = self
                .s
                .creature(id)
                .monster
                .as_ref()
                .is_some_and(|m| m.spawned_this_turn);
            if !spawned && !idle.contains(&id) {
                monsters::perform_move(self, id)?;
            }
            self.check_win_condition()?;
            if !self.s.in_progress {
                return Ok(());
            }
        }
        // EndEnemyTurn (`CombatManager.cs:1073`) -> EndEnemyTurnInternal (`:1696`).
        let enemies = Creatures::from_slice(&self.s.enemies);
        self.before_side_turn_end(Side::Enemy, &enemies)?;
        self.end_of_turn_cleanup();
        self.after_side_turn_end(Side::Enemy, &enemies)?;
        self.check_win_condition()?;
        if !self.hooked().is_ending() {
            self.switch_sides();
        }
        Ok(())
    }

    /// `SwitchFromPlayerToEnemySide` (`CombatManager.cs:1825-1853`): re-evaluate
    /// `ShouldTakeExtraTurn`, switch sides (back to the player for an extra turn), then run
    /// `AfterTakingExtraTurn`.
    fn switch_from_player_to_enemy_side(&mut self) -> Flow {
        self.s.player.extra_turn = self.hooked().should_take_extra_turn();
        self.switch_sides();
        if self.s.player.extra_turn {
            self.after_taking_extra_turn()?;
        }
        Ok(())
    }

    /// `SwitchSides` (`CombatManager.cs:1857`): an extra turn keeps the player side, counts
    /// the player's turn but not the round.
    fn switch_sides(&mut self) {
        if self.s.side == Side::Player && !self.s.player.extra_turn {
            self.s.side = Side::Enemy;
        } else {
            self.s.side = Side::Player;
            if !self.s.player.extra_turn {
                self.s.round += 1;
            }
            self.s.player.turn_number += 1;
        }
        // Creature.OnSideSwitch.
        let s = &mut *self.s;
        for id in s.allies.iter().chain(&s.enemies) {
            if let Some(m) = s.creatures[id.index()].monster.as_mut() {
                m.spawned_this_turn = false;
            }
        }
    }

    /// `CheckWinCondition` (`CombatManager.cs:1387`): process a pending loss, or end the
    /// combat as won once it is ending. Returns whether combat ended here.
    #[inline]
    pub(crate) fn check_win_condition(&mut self) -> Flow<bool> {
        if !self.s.pending_loss && !self.hooked().is_ending() {
            return Ok(false);
        }
        self.end_combat()
    }

    /// [`Engine::check_win_condition`] once the combat ends: the loss, or the victory.
    #[inline(never)]
    fn end_combat(&mut self) -> Flow<bool> {
        if self.s.pending_loss {
            // ProcessPendingLoss: no AfterCombatEnd hooks on a loss.
            self.s.in_progress = false;
            self.s.result = Some(CombatResult::Lost);
            self.emit(Event::Lost);
            return Ok(true);
        }
        if self.hooked().is_ending() {
            // EndCombatInternal (`CombatManager.cs:1297-1330`). A dead player is revived first
            // in multiplayer only: here a dead player means the loss above.
            self.s.in_progress = false;
            self.s.player.phase = TurnPhase::None;
            self.s.result = Some(CombatResult::Won);
            self.after_combat_end()?;
            self.s.history.clear();
            // `CombatRoom.OnCombatEnded`.
            self.s.gold_proportion = self.s.encounter.calculate_gold_proportion(self.s);
            self.tear_down_player();
            self.after_combat_victory()?;
            self.emit(Event::Won);
            return Ok(true);
        }
        Ok(false)
    }

    /// `Player.AfterCombatEnd` (`Player.cs:837-842`): remove the player's powers without
    /// callbacks, clear the piles and pets (`PlayerCombatState.AfterCombatEnd`), lose all block.
    fn tear_down_player(&mut self) {
        self.s.clear_piles();
        self.s.player.pets.clear();
        let player = self.s.player.creature;
        self.s.remove_all_powers(player);
        self.s.creature_mut(player).block = 0;
    }
}
