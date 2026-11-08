//! The fight's input state machine: what a key, a tap or a drag means, as the engine's actions.
//!
//! [`Ui`] holds the client's half-made choices (a card waiting for its target, a selected
//! potion, a dragged card, the keyboard focus) and turns an [`Input`] into [`Effect`]s: an
//! action for the run to apply, an error line, or [`Effect::Pass`] when the input is not the
//! fight's (a menu digit, the deck key). It never applies an action itself and never decides
//! what is legal: `Run::legal_actions` does, and the text of a refusal comes from the
//! engine's reason.
//!
//! Buttons are numbered by [`ids`], so a tap names a row, not a position on a terminal.

use sts2_core::combat::{CardId, CreatureId, Decision, IncomingPreview, Status};
use sts2_core::run::{Run, RunAction, RunDecision};
use sts2_core::{Action, Combat};

use crate::combat::{self, POTION_KEYS};
use crate::describe::Describer;
use crate::model::{ButtonId, Input, Key, Screen, TopBar};
use crate::topbar::{self, Waiting};
use crate::{names, text};

mod room;
mod sheet;

/// What a combat target is being chosen for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pending {
    /// Play this card.
    Card(CardId),
    /// Use the potion in this belt slot.
    Potion(usize),
}

/// What the next input means in a fight.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Pick a card from the hand, or an answer to the pending decision.
    #[default]
    Idle,
    /// A card or potion is chosen and needs a target.
    Target(Pending),
    /// The potion in this belt slot is selected and needs no target: its row asks in place,
    /// and Enter, its key again or a tap on it uses it; `t` throws it away; anything else
    /// puts it back.
    Potion(usize),
    /// A card that needs no target is being dragged: a drop on the battlefield (or `Enter`)
    /// plays it, a drop anywhere else puts it back.
    Armed(CardId),
}

/// What the keyboard focus can rest on in a fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    Potion(usize),
    Card(CardId),
    /// The `n`th (1-based) answer of a card choice.
    Choice(usize, CardId),
    Creature(CreatureId),
    EndTurn,
    /// The `n`th (1-based) answer of a card choice, when it is Done.
    Done(usize),
}

/// What [`Ui::input`] asks the caller to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Apply this action to the run (it came from `legal_actions` or `discard_actions`).
    Apply(RunAction),
    /// Show this error line.
    Error(String),
    /// The run's end screen asks for this.
    End(crate::rooms::EndAction),
    /// A menu row the client carries out itself (help, settings, quit, throwing a potion
    /// away).
    Menu(crate::sheets::Entry),
    /// The input is not the fight's: the caller handles it.
    Pass,
}

/// The ids of the fight's buttons.
pub mod ids {
    use sts2_core::combat::CreatureId;

    use crate::model::ButtonId;

    /// End Turn.
    pub const END_TURN: ButtonId = ButtonId(999);
    const POTION: u32 = 100;
    const CREATURE: u32 = 1000;

    /// The `i`th (0-based) row of the hand or of a card choice.
    #[must_use]
    pub fn row(i: usize) -> ButtonId {
        ButtonId(u32::try_from(i + 1).unwrap_or(u32::MAX))
    }

    /// The potion in belt slot `slot`.
    #[must_use]
    pub fn potion(slot: usize) -> ButtonId {
        ButtonId(POTION + u32::try_from(slot).unwrap_or(0))
    }

    /// A creature as a target.
    #[must_use]
    pub const fn creature(id: CreatureId) -> ButtonId {
        ButtonId(CREATURE + id.0)
    }

    /// What `id` names.
    #[must_use]
    pub const fn decode(id: ButtonId) -> Hit {
        match id.0 {
            999 => Hit::EndTurn,
            n @ 1000.. => Hit::Creature(CreatureId(n - CREATURE)),
            n @ POTION..=998 => Hit::Potion((n - POTION) as usize),
            n => Hit::Row((n as usize).saturating_sub(1)),
        }
    }

    /// A fight button, decoded.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Hit {
        EndTurn,
        Creature(CreatureId),
        Potion(usize),
        Row(usize),
    }
}

/// Which list the keyboard focus moves in, so a focus kept from one frame still means
/// something in the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pane {
    /// The fight's hand (with the potions and End Turn) in this round.
    Hand(u32),
    /// The targets of the card or potion waiting for one.
    Targets,
    /// A card choice of the fight.
    Choice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Focus {
    pane: Pane,
    item: Item,
    pos: usize,
}

/// The fight's client state.
#[derive(Clone, Debug, Default)]
pub struct Ui {
    mode: Mode,
    /// The enemy the last card or potion was aimed at in this fight (`Space` aims there again).
    last_target: Option<CreatureId>,
    /// What the enemy turn was going to do when the last play began: kept for a card choice of
    /// the fight (the engine has no preview while one is open).
    turn_incoming: Option<IncomingPreview>,
    focus: Option<Focus>,
    /// The pane and the aim of the action about to be applied.
    applying: Option<(Option<Pane>, Option<CreatureId>)>,
    /// The rooms outside a fight: the card reward on show, the button that asks.
    room: room::State,
    /// The sheet over the screen (the run menu, a list) and how far it is scrolled.
    sheet: sheet::State,
}

impl Ui {
    #[must_use]
    pub const fn mode(&self) -> &Mode {
        &self.mode
    }

    #[must_use]
    pub const fn last_target(&self) -> Option<CreatureId> {
        self.last_target
    }

    #[must_use]
    pub const fn turn_incoming(&self) -> Option<&IncomingPreview> {
        self.turn_incoming.as_ref()
    }

    /// Puts a half-chosen card or potion back.
    pub const fn cancel(&mut self) {
        self.mode = Mode::Idle;
    }

    /// The fight as a screen: the selection (the waiting or dragged card) and the focus feed
    /// the builder.
    #[must_use]
    pub fn screen(&self, run: &Run, describer: &Describer) -> Option<Screen> {
        let combat = run.combat()?;
        let (selected, waiting) = match &self.mode {
            Mode::Target(Pending::Card(card)) | Mode::Armed(card) => (Some(*card), true),
            _ => match self.focused(run) {
                Some(Item::Card(card)) if self.mode == Mode::Idle => (Some(card), false),
                _ => (None, false),
            },
        };
        let pending = match &self.mode {
            Mode::Target(pending) => Some(*pending),
            _ => None,
        };
        let target = pending.as_ref().and(self.last_target);
        let kept_incoming = self.turn_incoming.clone();
        let mut screen = combat::screen(
            combat,
            describer,
            &combat::Selection {
                selected,
                waiting,
                pending,
                armed: match self.mode {
                    Mode::Armed(card) => Some(card),
                    _ => None,
                },
                target,
                focus: self.focused(run),
                kept_incoming,
            },
        );
        screen.top = Some(self.top_bar(run, describer));
        Some(screen)
    }

    /// The bar above every screen, in a fight or out of it: HP, gold, floor, stars and the
    /// belt, with the potion that waits for its confirmation or its target selected.
    #[must_use]
    pub fn top_bar(&self, run: &Run, describer: &Describer) -> TopBar {
        let waiting = match self.mode {
            Mode::Potion(slot) => Some(Waiting {
                slot,
                confirm: true,
            }),
            Mode::Target(Pending::Potion(slot)) => Some(Waiting {
                slot,
                confirm: false,
            }),
            _ => None,
        };
        topbar::build(run, describer, waiting)
    }

    /// Handles `input` in the fight.
    pub fn input(&mut self, run: &Run, describer: &Describer, input: &Input) -> Vec<Effect> {
        if let Mode::Potion(slot) = self.mode {
            return self.potion_waits(run, slot, input);
        }
        match *input {
            Input::Key(key) => self.key(run, describer, key),
            Input::Tap(id) => self.tap(run, describer, id),
            Input::Primary => self.primary(run),
            Input::Back => {
                if self.mode == Mode::Idle {
                    pass()
                } else {
                    self.cancel();
                    done()
                }
            }
            Input::Arm(id) => self.arm(run, describer, id),
            Input::Drop => self.play_armed(run),
            Input::Info(_) | Input::Page(_) => pass(),
        }
    }

    /// A selected potion takes every input: its key again or Enter uses it, `t` throws it away,
    /// another potion's key selects that one, anything else puts it back.
    fn potion_waits(&mut self, run: &Run, slot: usize, input: &Input) -> Vec<Effect> {
        match input {
            Input::Page(_) | Input::Info(_) => pass(),
            Input::Primary | Input::Key(Key::Enter | Key::Space) => self.use_selected_potion(run),
            Input::Tap(id) => match ids::decode(*id) {
                ids::Hit::Potion(s) if s == slot => self.use_selected_potion(run),
                ids::Hit::Potion(other) => self.select_potion(run, other),
                _ => {
                    self.cancel();
                    done()
                }
            },
            Input::Key(key) => match (key, potion_slot(*key)) {
                (Key::Char('t'), _) => {
                    self.cancel();
                    self.pick_discard(run, slot + 1)
                }
                (Key::Esc | Key::Backspace, _) => {
                    self.cancel();
                    done()
                }
                (_, Some(other)) => self.select_potion(run, other),
                (Key::Up | Key::Down | Key::Left | Key::Right, _) => done(),
                _ => {
                    self.cancel();
                    done()
                }
            },
            Input::Back | Input::Arm(_) | Input::Drop => {
                self.cancel();
                done()
            }
        }
    }

    fn key(&mut self, run: &Run, describer: &Describer, key: Key) -> Vec<Effect> {
        if let Mode::Armed(_) = self.mode {
            match key {
                Key::Enter | Key::Space => return self.play_armed(run),
                Key::Up | Key::Down | Key::Left | Key::Right | Key::Esc => {}
                _ => self.cancel(),
            }
        }
        if let Some(slot) = potion_slot(key) {
            return self.select_potion(run, slot);
        }
        let Some(combat) = run.combat() else {
            return pass();
        };
        match key {
            Key::Digit(d) => {
                let index = if d == 0 { 10 } else { usize::from(d) };
                match self.mode.clone() {
                    Mode::Target(pending) => self.pick_target(run, &pending, index),
                    _ if combat.status() == Status::Decision => pick_decision(run, index),
                    _ => self.pick_card(run, describer, index),
                }
            }
            Key::Backspace | Key::Esc => {
                self.cancel();
                done()
            }
            Key::Char('e') if combat.status() == Status::PlayerTurn => self.end_turn(run),
            Key::Space if matches!(self.mode, Mode::Target(_)) => self.repeat_target(run),
            Key::Enter | Key::Space => self.activate(run, describer),
            Key::Up | Key::Left => self.focus_move(run, -1),
            Key::Down | Key::Right => self.focus_move(run, 1),
            _ => pass(),
        }
    }

    fn tap(&mut self, run: &Run, describer: &Describer, id: ButtonId) -> Vec<Effect> {
        let Some(combat) = run.combat() else {
            return pass();
        };
        match ids::decode(id) {
            ids::Hit::EndTurn => self.end_turn(run),
            ids::Hit::Creature(creature) => self.target(run, creature),
            ids::Hit::Potion(slot) => self.select_potion(run, slot),
            ids::Hit::Row(i) if combat.status() == Status::Decision => {
                self.cancel();
                pick_decision(run, i + 1)
            }
            ids::Hit::Row(i) => match combat.state().player.hand.get(i) {
                Some(&card) => self.click_card(run, describer, card),
                None => done(),
            },
        }
    }

    /// The main button: End Turn, a card choice's Done, or using the selected potion.
    fn primary(&mut self, run: &Run) -> Vec<Effect> {
        let Some(combat) = run.combat() else {
            return pass();
        };
        if combat.status() == Status::Decision {
            let done_at = combat::choice_actions(combat)
                .iter()
                .position(|a| matches!(a, Action::Resolve(Decision::Done)));
            return done_at.map_or_else(done, |at| {
                self.cancel();
                pick_decision(run, at + 1)
            });
        }
        self.end_turn(run)
    }

    fn end_turn(&mut self, run: &Run) -> Vec<Effect> {
        if !in_player_turn(run) {
            return done();
        }
        self.cancel();
        apply(RunAction::Combat(Action::EndTurn))
    }

    /// Plays `card` of the hand: at once when it has one way to be played, else it waits for
    /// its target; a card with no legal play says why.
    fn play_card(&mut self, run: &Run, describer: &Describer, card: CardId) -> Vec<Effect> {
        match card_targets(run, card).as_slice() {
            [] => reject_play(run, describer, card),
            [only] => apply(RunAction::Combat(Action::PlayCard {
                card,
                target: *only,
            })),
            _ => {
                self.mode = Mode::Target(Pending::Card(card));
                done()
            }
        }
    }

    /// A press on a card of the hand: one press plays it, or puts it up for its target; a press
    /// on the card waiting for a target puts it back.
    fn click_card(&mut self, run: &Run, describer: &Describer, card: CardId) -> Vec<Effect> {
        if matches!(self.mode, Mode::Target(Pending::Card(c)) if c == card) {
            self.cancel();
            return done();
        }
        self.cancel();
        if !in_player_turn(run) || !in_hand(run, card) {
            return done();
        }
        self.play_card(run, describer, card)
    }

    /// A drag starts on a card: it waits for a target, or (one way to play it) for a drop on
    /// the battlefield.
    fn arm(&mut self, run: &Run, describer: &Describer, id: ButtonId) -> Vec<Effect> {
        self.cancel();
        let Some(combat) = run.combat() else {
            return pass();
        };
        let ids::Hit::Row(i) = ids::decode(id) else {
            return done();
        };
        let Some(&card) = combat.state().player.hand.get(i) else {
            return done();
        };
        if !in_player_turn(run) {
            return done();
        }
        match card_targets(run, card).as_slice() {
            [] => reject_play(run, describer, card),
            [_] => {
                self.mode = Mode::Armed(card);
                done()
            }
            _ => {
                self.mode = Mode::Target(Pending::Card(card));
                done()
            }
        }
    }

    /// Plays the dragged card that has one way to be played.
    fn play_armed(&mut self, run: &Run) -> Vec<Effect> {
        let Mode::Armed(card) = self.mode else {
            return done();
        };
        if let &[target] = card_targets(run, card).as_slice() {
            apply(RunAction::Combat(Action::PlayCard { card, target }))
        } else {
            self.cancel();
            done()
        }
    }

    /// `creature` is tapped (or a card dropped on it) as the target of what is selected.
    fn target(&mut self, run: &Run, creature: CreatureId) -> Vec<Effect> {
        match self.mode.clone() {
            Mode::Armed(card) => match card_targets(run, card).as_slice() {
                &[Some(only)] if only != creature => error("invalid target"),
                _ => self.play_armed(run),
            },
            Mode::Target(pending) => target_creature(run, &pending, creature),
            _ => done(),
        }
    }

    /// `Space` while a card waits for its target: plays it at the enemy the last card or potion
    /// was aimed at in this fight.
    fn repeat_target(&self, run: &Run) -> Vec<Effect> {
        let Mode::Target(pending) = &self.mode else {
            return done();
        };
        self.last_target.map_or_else(
            || error("no target used yet this fight"),
            |target| target_creature(run, pending, target),
        )
    }

    fn pick_card(&mut self, run: &Run, describer: &Describer, index: usize) -> Vec<Effect> {
        self.cancel();
        let Some(&card) = run
            .combat()
            .and_then(|c| c.state().player.hand.get(index - 1))
        else {
            return error(format!("no card {index} in hand"));
        };
        self.play_card(run, describer, card)
    }

    fn pick_target(&mut self, run: &Run, pending: &Pending, index: usize) -> Vec<Effect> {
        let combat = run.combat();
        let Some(&enemy) = combat.and_then(|c| c.state().enemies.get(index - 1)) else {
            // The selected card's own number again (when no enemy has it) cancels.
            let own = match (pending, combat) {
                (Pending::Card(card), Some(c)) => {
                    c.state().player.hand.get(index - 1) == Some(card)
                }
                _ => false,
            };
            if own {
                self.cancel();
                return done();
            }
            return error(format!("no enemy {index}"));
        };
        target_creature(run, pending, enemy)
    }

    fn pick_discard(&mut self, run: &Run, index: usize) -> Vec<Effect> {
        let action = RunAction::DiscardPotion(index - 1);
        if run.discard_actions().contains(&action) {
            apply(action)
        } else {
            self.cancel();
            error(format!("no potion in slot {index}"))
        }
    }

    /// A press on the potion in `slot` (its key, Enter on its row, a tap): a potion with one
    /// living target is used there at once; one with more waits for its target; one that needs
    /// none is selected and asks in place (the same press again uses it).
    fn select_potion(&mut self, run: &Run, slot: usize) -> Vec<Effect> {
        if self.mode == Mode::Potion(slot) {
            return self.use_selected_potion(run);
        }
        self.cancel();
        let uses: Vec<RunAction> = potion_actions(run)
            .into_iter()
            .filter(|(s, _)| *s == slot)
            .map(|(_, a)| a)
            .collect();
        let enemies: Vec<CreatureId> = run
            .combat()
            .map(|c| c.state().enemies.clone())
            .unwrap_or_default();
        match uses.as_slice() {
            [] => error(run.potions().get(slot).copied().flatten().map_or_else(
                || format!("no potion in slot {}", slot + 1),
                |id| format!("{} cannot be used now", names::potion(id)),
            )),
            // One living enemy to throw it at: thrown at once. A potion aimed at yourself
            // (the engine lists the player as its one target) asks like one with no target.
            [
                only @ RunAction::Combat(Action::UsePotion {
                    target: Some(t), ..
                }),
            ] if enemies.contains(t) => apply(only.clone()),
            [_] => {
                self.mode = Mode::Potion(slot);
                self.focus_on(run, Item::Potion(slot));
                done()
            }
            _ => {
                self.mode = Mode::Target(Pending::Potion(slot));
                done()
            }
        }
    }

    /// Uses the selected potion (the one that needs no target).
    fn use_selected_potion(&mut self, run: &Run) -> Vec<Effect> {
        let Mode::Potion(slot) = self.mode else {
            return done();
        };
        self.cancel();
        match potion_actions(run).into_iter().find(|(s, _)| *s == slot) {
            Some((_, action)) => apply(action),
            None => error("that potion cannot be used now"),
        }
    }

    // ---- the keyboard focus ----

    fn pane(&self, run: &Run) -> Option<Pane> {
        let combat = run.combat()?;
        Some(match &self.mode {
            Mode::Target(_) => Pane::Targets,
            _ if combat.status() == Status::Decision => Pane::Choice,
            _ => Pane::Hand(combat.state().round),
        })
    }

    /// What the focus can rest on, top to bottom: the potions, the hand and End Turn; the
    /// targets while a card waits for one; the answers of a card choice.
    #[must_use]
    pub fn items(&self, run: &Run) -> Vec<Item> {
        let Some(combat) = run.combat() else {
            return Vec::new();
        };
        match (self.pane(run), &self.mode) {
            (Some(Pane::Targets), Mode::Target(pending)) => targets_of(run, pending)
                .into_iter()
                .map(Item::Creature)
                .collect(),
            (Some(Pane::Choice), _) => {
                let options = decision_options(run);
                let mut items: Vec<Item> = options
                    .iter()
                    .enumerate()
                    .filter_map(|(i, a)| match a {
                        Action::Resolve(Decision::Pick(card) | Decision::Unpick(card)) => {
                            Some(Item::Choice(i + 1, *card))
                        }
                        _ => None,
                    })
                    .collect();
                if let Some(done_at) = options
                    .iter()
                    .position(|a| matches!(a, Action::Resolve(Decision::Done)))
                {
                    items.push(Item::Done(done_at + 1));
                }
                items
            }
            (Some(Pane::Hand(_)), _) => {
                let mut items: Vec<Item> = run
                    .potions()
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, p)| p.map(|_| Item::Potion(slot)))
                    .collect();
                items.extend(combat.state().player.hand.iter().map(|&c| Item::Card(c)));
                if in_player_turn(run) {
                    items.push(Item::EndTurn);
                }
                items
            }
            _ => Vec::new(),
        }
    }

    /// What the keyboard focus rests on: where it was put, if that is still there; its place in
    /// the list if what was there went (the card played); else the screen's default: the first
    /// card that can be played, the last target aimed at, the first choice.
    #[must_use]
    pub fn focused(&self, run: &Run) -> Option<Item> {
        let items = self.items(run);
        if items.is_empty() {
            return None;
        }
        let pane = self.pane(run)?;
        if let Some(focus) = self.focus.filter(|f| f.pane == pane) {
            if items.contains(&focus.item) {
                return Some(focus.item);
            }
            return items.get(focus.pos.min(items.len() - 1)).copied();
        }
        let default = match pane {
            Pane::Hand(_) => items
                .iter()
                .find(|&&t| playable(run, t))
                .or_else(|| items.iter().find(|t| **t == Item::EndTurn)),
            Pane::Targets => items
                .iter()
                .find(|t| matches!(t, Item::Creature(c) if Some(*c) == self.last_target)),
            Pane::Choice => None,
        };
        default.or_else(|| items.first()).copied()
    }

    /// Puts the focus on `item` (a tap landed on it), if it can be focused now.
    pub fn focus_on(&mut self, run: &Run, item: Item) {
        let items = self.items(run);
        if let (Some(pane), Some(pos)) = (self.pane(run), items.iter().position(|t| *t == item)) {
            self.focus = Some(Focus { pane, item, pos });
        }
    }

    fn focus_move(&mut self, run: &Run, by: isize) -> Vec<Effect> {
        let items = self.items(run);
        let Some(pane) = self.pane(run).filter(|_| !items.is_empty()) else {
            return done();
        };
        let at = self
            .focused(run)
            .and_then(|f| items.iter().position(|t| *t == f))
            .unwrap_or(0);
        let n = items.len().cast_signed();
        let pos = (at.cast_signed() + by).rem_euclid(n).cast_unsigned();
        self.focus = Some(Focus {
            pane,
            item: items[pos],
            pos,
        });
        done()
    }

    /// Enter: presses what the focus rests on.
    fn activate(&mut self, run: &Run, describer: &Describer) -> Vec<Effect> {
        let Some(item) = self.focused(run) else {
            return done();
        };
        let hand = |card: CardId| {
            run.combat()
                .and_then(|c| c.state().player.hand.iter().position(|&h| h == card))
        };
        let id = match item {
            Item::Card(card) => match hand(card) {
                Some(i) => ids::row(i),
                None => return done(),
            },
            Item::Choice(n, _) | Item::Done(n) => ids::row(n - 1),
            Item::Creature(c) => ids::creature(c),
            Item::Potion(slot) => ids::potion(slot),
            Item::EndTurn => ids::END_TURN,
        };
        self.tap(run, describer, id)
    }

    /// Settles the hand's focus after a play: a card that is gone gives its focus to the card
    /// now in its slot (the last card, when it was the last), or the playable card nearest
    /// that; a card that stays keeps it. End Turn takes it only when no card can be played.
    fn settle_hand_focus(&mut self, run: &Run) {
        let Some(focus) = self.focus else {
            return;
        };
        let items = self.items(run);
        let Some(pane) = self.pane(run) else {
            return;
        };
        let any_playable = items.iter().any(|&t| playable(run, t));
        let cards: Vec<usize> = (0..items.len())
            .filter(|&i| matches!(items[i], Item::Card(_)))
            .collect();
        let end_turn = items.iter().position(|t| *t == Item::EndTurn);
        let kept = items.iter().position(|t| *t == focus.item);
        let pos = match (kept, cards.first().zip(cards.last())) {
            // A potion or End Turn, or a card still in the hand while some card can be played.
            (Some(at), _) if !matches!(focus.item, Item::Card(_)) || any_playable => at,
            (_, Some((&first, &last))) if any_playable => {
                let slot = focus.pos.clamp(first, last);
                cards
                    .iter()
                    .copied()
                    .filter(|&i| playable(run, items[i]))
                    .min_by_key(|&i| (i.abs_diff(slot), i < slot))
                    .unwrap_or(slot)
            }
            _ => end_turn
                .or(kept)
                .unwrap_or_else(|| focus.pos.min(items.len().saturating_sub(1))),
        };
        self.focus = items.get(pos).map(|&item| Focus { pane, item, pos });
    }

    // ---- around an applied action ----

    /// Call before `action` is applied to `run`: notes what the incoming line showed and
    /// where the action is aimed.
    pub fn before_apply(&mut self, run: &Run, action: &RunAction) {
        let aimed = match action {
            RunAction::Combat(
                Action::PlayCard {
                    target: Some(t), ..
                }
                | Action::UsePotion {
                    target: Some(t), ..
                },
            ) => run
                .combat()
                .filter(|c| c.state().enemies.contains(t))
                .map(|_| *t),
            _ => None,
        };
        if let Some(incoming) = run.combat().and_then(Combat::incoming) {
            self.turn_incoming = Some(incoming);
        }
        self.applying = Some((self.pane(run), aimed));
    }

    /// Call after the action applied: the fight's memory moves on.
    pub fn after_apply(&mut self, run: &Run) {
        self.room_applied(run);
        let Some((before, aimed)) = self.applying.take() else {
            return;
        };
        if run.combat().is_none() {
            self.last_target = None;
            self.turn_incoming = None;
        } else if aimed.is_some() {
            self.last_target = aimed;
        }
        // Another screen (or another turn) starts with its own default focus; the hand keeps
        // its focus on a card that can still be played.
        let now = self.pane(run);
        if now != before {
            self.focus = None;
        } else if matches!(now, Some(Pane::Hand(_))) {
            self.settle_hand_focus(run);
        }
    }
}

/// The keys of the fight, for the help: the title of each group and its rows.
pub const HELP: &[(&str, &[(&str, &str)])] = &[
    ("Your turn", &[("e", "end the turn")]),
    (
        "Choosing",
        &[(
            "z c b n g",
            "select the potion in that belt slot, as the potion row prints them: a potion \
             with one living target is used at once, one with more waits for its target, one \
             that needs none asks in place (Enter or the key again uses it, t throws it away)",
        )],
    ),
    (
        "Choosing a target",
        &[("Space", "play it at the last target used this fight")],
    ),
    (
        "Dragged card",
        &[("Enter/Space", "play it (any other key drops it)")],
    ),
    (
        "A potion waits",
        &[
            (
                "Enter",
                "use the selected potion (its own key again does too)",
            ),
            ("t", "throw the selected potion away"),
            ("Esc", "put the potion back"),
            ("z c b n g", "select that potion instead"),
        ],
    ),
];

const fn done() -> Vec<Effect> {
    Vec::new()
}

fn pass() -> Vec<Effect> {
    vec![Effect::Pass]
}

fn apply(action: RunAction) -> Vec<Effect> {
    vec![Effect::Apply(action)]
}

fn pick_decision(run: &Run, index: usize) -> Vec<Effect> {
    decision_options(run).get(index - 1).map_or_else(
        || error(format!("no option {index}")),
        |action| apply(RunAction::Combat(action.clone())),
    )
}

fn error(message: impl Into<String>) -> Vec<Effect> {
    vec![Effect::Error(message.into())]
}

/// The belt slot `key` selects, if it is a potion key.
#[must_use]
pub fn potion_slot(key: Key) -> Option<usize> {
    match key {
        Key::Char(c) => POTION_KEYS.iter().position(|&k| k == c),
        _ => None,
    }
}

fn in_player_turn(run: &Run) -> bool {
    run.combat()
        .is_some_and(|c| c.status() == Status::PlayerTurn)
}

fn in_hand(run: &Run, card: CardId) -> bool {
    run.combat()
        .is_some_and(|c| c.state().player.hand.contains(&card))
}

/// The action that follows on its own after one applied: a reward screen with nothing left on
/// it leaves, as Leave is all it would offer.
#[must_use]
pub fn follow_up(run: &Run) -> Option<RunAction> {
    matches!(run.pending_decision(), RunDecision::Rewards(left) if left.is_empty())
        .then_some(RunAction::Proceed)
        .filter(|a| run.legal_actions().contains(a))
}

/// The legal answers to the combat's pending card choice, in display order.
#[must_use]
pub fn decision_options(run: &Run) -> Vec<Action> {
    run.combat()
        .filter(|c| c.status() == Status::Decision)
        .map(combat::choice_actions)
        .unwrap_or_default()
}

/// The belt slots whose potion can be used now, with the actions that use them (one per
/// target).
#[must_use]
pub fn potion_actions(run: &Run) -> Vec<(usize, RunAction)> {
    run.legal_actions()
        .into_iter()
        .filter_map(|a| match a {
            RunAction::UsePotion(slot) | RunAction::Combat(Action::UsePotion { slot, .. }) => {
                Some((slot, a))
            }
            _ => None,
        })
        .collect()
}

/// The targets `card` can be played on now (`None`: it takes no target).
fn card_targets(run: &Run, card: CardId) -> Vec<Option<CreatureId>> {
    run.legal_actions()
        .into_iter()
        .filter_map(|a| match a {
            RunAction::Combat(Action::PlayCard { card: c, target }) if c == card => Some(target),
            _ => None,
        })
        .collect()
}

/// The creatures `pending` can be aimed at now: the enemies in their order, then Osty or the
/// player.
fn targets_of(run: &Run, pending: &Pending) -> Vec<CreatureId> {
    let Some(combat) = run.combat() else {
        return Vec::new();
    };
    let aimed: Vec<CreatureId> = run
        .legal_actions()
        .into_iter()
        .filter_map(|a| match (a, pending) {
            (
                RunAction::Combat(Action::PlayCard {
                    card,
                    target: Some(t),
                }),
                Pending::Card(c),
            ) if card == *c => Some(t),
            (
                RunAction::Combat(Action::UsePotion {
                    slot,
                    target: Some(t),
                }),
                Pending::Potion(s),
            ) if slot == *s => Some(t),
            _ => None,
        })
        .collect();
    let enemies = &combat.state().enemies;
    let mut targets: Vec<CreatureId> = enemies
        .iter()
        .copied()
        .filter(|e| aimed.contains(e))
        .collect();
    targets.extend(aimed.iter().copied().filter(|t| !enemies.contains(t)));
    targets
}

/// Whether `item` is a card of the hand the engine lets the player play now (a 0-cost card
/// with no energy left counts).
fn playable(run: &Run, item: Item) -> bool {
    matches!(item, Item::Card(c) if !card_targets(run, c).is_empty())
}

fn target_creature(run: &Run, pending: &Pending, creature: CreatureId) -> Vec<Effect> {
    let target = Some(creature);
    let action = RunAction::Combat(match *pending {
        Pending::Card(card) => Action::PlayCard { card, target },
        Pending::Potion(slot) => Action::UsePotion { slot, target },
    });
    if run.legal_actions().contains(&action) {
        apply(action)
    } else {
        error("invalid target")
    }
}

/// Reports why a card that has no legal play is rejected, as the engine words the reason, with
/// the key that gets past it when there is one (`e` when the energy is spent).
fn reject_play(run: &Run, describer: &Describer, card: CardId) -> Vec<Effect> {
    let Some(combat) = run.combat() else {
        return done();
    };
    let state = combat.state();
    let reason = state.unplayable_reason(card).map_or_else(
        || "it has no target now".to_owned(),
        |r| {
            let text = text::plain(&describer.unplayable(state, r));
            text.trim_end_matches(['.', '!']).to_owned()
        },
    );
    let fix = if reason.contains("Energy") && in_player_turn(run) {
        " · e ends the turn"
    } else {
        ""
    };
    error(format!("cannot play: {reason}{fix}"))
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use sts2_core::Character;
    use sts2_core::run::{Act1, UnlockState};

    use super::*;
    use crate::model::{Block, ButtonState, Preview};

    fn fight() -> (Run, Describer) {
        let mut run = Run::new(
            Character::Necrobinder,
            "RL2S2",
            0,
            UnlockState::all(),
            Act1::Random,
        )
        .expect("run")
        .with_events();
        for _ in 0..60 {
            if run.combat().is_some() {
                break;
            }
            let action = run.legal_actions().remove(0);
            run.apply(action).expect("legal");
        }
        assert!(run.combat().is_some(), "a fight within 60 actions");
        (run, Describer::new(Rc::new(text::Localization::empty())))
    }

    fn effects(ui: &mut Ui, run: &Run, describer: &Describer, input: &Input) -> Vec<Effect> {
        ui.input(run, describer, input)
    }

    #[test]
    fn end_turn_is_one_tap_or_one_key() {
        let (run, describer) = fight();
        let end = vec![Effect::Apply(RunAction::Combat(Action::EndTurn))];
        let mut ui = Ui::default();
        assert_eq!(
            effects(&mut ui, &run, &describer, &Input::Tap(ids::END_TURN)),
            end
        );
        assert_eq!(
            effects(&mut ui, &run, &describer, &Input::Key(Key::Char('e'))),
            end
        );
        assert_eq!(effects(&mut ui, &run, &describer, &Input::Primary), end);
    }

    #[test]
    fn a_key_the_fight_does_not_own_passes() {
        let (run, describer) = fight();
        let mut ui = Ui::default();
        assert_eq!(
            effects(&mut ui, &run, &describer, &Input::Key(Key::Char('d'))),
            vec![Effect::Pass]
        );
        assert_eq!(
            effects(&mut ui, &run, &describer, &Input::Key(Key::Tab)),
            vec![Effect::Pass]
        );
    }

    #[test]
    fn a_digit_past_the_hand_is_an_error_and_a_potion_key_without_a_potion_too() {
        let (run, describer) = fight();
        let mut ui = Ui::default();
        let hand = run.combat().expect("fight").state().player.hand.len();
        let past = u8::try_from(hand + 1).expect("small");
        assert_eq!(
            effects(
                &mut ui,
                &run,
                &describer,
                &Input::Key(Key::Digit(past % 10))
            ),
            vec![Effect::Error(format!("no card {past} in hand"))]
        );
        assert_eq!(
            effects(&mut ui, &run, &describer, &Input::Key(Key::Char('z'))),
            vec![Effect::Error("no potion in slot 1".to_owned())]
        );
        assert_eq!(ui.mode(), &Mode::Idle);
    }

    #[test]
    fn a_card_that_needs_a_target_waits_and_escape_puts_it_back() {
        let (run, describer) = fight();
        let combat = run.combat().expect("fight");
        let mut ui = Ui::default();
        // The first card that has more than one way to be played waits for its target; with
        // one enemy every attack is played at once, so find a card either way.
        let hand = combat.state().player.hand.clone();
        for (i, _) in hand.iter().enumerate() {
            let out = effects(&mut ui, &run, &describer, &Input::Tap(ids::row(i)));
            if matches!(ui.mode(), Mode::Target(_)) {
                assert_eq!(out, Vec::new());
                assert_eq!(
                    effects(&mut ui, &run, &describer, &Input::Key(Key::Esc)),
                    Vec::new()
                );
                assert_eq!(ui.mode(), &Mode::Idle);
                break;
            }
            assert!(matches!(
                out.as_slice(),
                [Effect::Apply(_) | Effect::Error(_)]
            ));
        }
    }

    #[test]
    fn the_screen_marks_the_focused_card_and_previews_what_it_does() {
        let (run, describer) = fight();
        let ui = Ui::default();
        let screen = ui.screen(&run, &describer).expect("a fight");
        let hand = screen
            .blocks
            .iter()
            .find_map(|b| match b {
                Block::List { rows, .. } => Some(rows),
                _ => None,
            })
            .expect("a hand");
        let focused = ui.focused(&run).expect("a focus");
        if let Item::Card(card) = focused {
            let at = run
                .combat()
                .expect("fight")
                .state()
                .player
                .hand
                .iter()
                .position(|&c| c == card)
                .expect("in hand");
            assert_eq!(hand[at].state, ButtonState::Selected);
        } else {
            assert_eq!(focused, Item::EndTurn);
        }
        // Only what is certain previews while a card is merely in focus.
        for block in &screen.blocks {
            if let Block::Enemy(enemy) = block {
                assert!(!matches!(
                    enemy.preview,
                    Some(Preview::Unchanged | Preview::Unknown)
                ));
            }
        }
    }

    #[test]
    fn the_bar_has_the_fights_hp_and_the_belt_slots_with_their_keys() {
        let (run, describer) = fight();
        let bar = Ui::default().top_bar(&run, &describer);
        let combat = run.combat().expect("fight");
        let state = combat.state();
        assert!(bar.fight);
        assert_eq!(bar.hp, state.creature(state.player.creature).hp);
        assert_eq!(
            bar.floor,
            u32::try_from(run.state().total_floor()).unwrap_or(0)
        );
        assert_eq!(bar.potions.len(), run.potions().len());
        for (slot, p) in bar.potions.iter().enumerate() {
            assert_eq!(p.key, POTION_KEYS.get(slot).copied());
            assert_eq!(p.id, ids::potion(slot));
            assert_eq!(p.empty, run.potions()[slot].is_none());
            assert!(!p.selected && !p.asks);
            assert_eq!(
                p.usable,
                potion_actions(&run).iter().any(|(s, _)| *s == slot)
            );
        }
    }

    #[test]
    fn the_bar_outside_a_fight_has_no_fight_flag() {
        let run = Run::new(
            Character::Necrobinder,
            "RL2S2",
            0,
            UnlockState::all(),
            Act1::Random,
        )
        .expect("run");
        let describer = Describer::new(Rc::new(text::Localization::empty()));
        let bar = Ui::default().top_bar(&run, &describer);
        assert!(!bar.fight);
        assert_eq!(bar.hp, run.state().hp);
        assert_eq!(bar.gold, run.state().gold);
    }

    #[test]
    fn the_selected_potion_asks_or_waits_for_its_target() {
        let (run, describer) = fight();
        let mut ui = Ui {
            mode: Mode::Potion(1),
            ..Ui::default()
        };
        let bar = ui.top_bar(&run, &describer);
        assert_eq!(
            bar.potions
                .iter()
                .map(|p| (p.selected, p.asks))
                .collect::<Vec<_>>()[..2],
            [(false, false), (true, true)]
        );
        ui.mode = Mode::Target(Pending::Potion(0));
        let bar = ui.top_bar(&run, &describer);
        assert_eq!(
            (bar.potions[0].selected, bar.potions[0].asks),
            (true, false)
        );
        assert!(!bar.potions[1].selected);
    }
}
