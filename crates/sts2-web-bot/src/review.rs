//! The run review (WEB46): the decisions of a finished run where the bot most disagrees with
//! the player, from the bot's own scores, one decision a step in the page's Web Worker.
//!
//! A fight's move is compared by the search's scores (its chance of winning the fight plus the
//! HP kept, over sampled worlds), a run choice by the run policy's probabilities. What has no
//! score (a forced move, a card choice answered by rule, a choice by a hand rule) is not a pick.

use serde_json::{Value, json};
use sts2_core::run::{RoomType, RunAction, RunDecision};
use sts2_diff::honest::Bot;
use sts2_diff::honest::advice::{ChoiceGap, FightGap, score_permille};
use sts2_view::ui::follow_up;
use sts2_web::{Game, parse_modifiers};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{bot, decision_word, outcome, parse_actions, shown, view_of};

/// The picks a review ends on.
const PICKS: usize = 3;
/// The disagreements kept while the review runs.
const KEEP: usize = 40;
/// Fight gaps and run-choice gaps are different numbers, so a pick's rank needs a rule: a
/// fight's gap counts as it is (thousandths of a fight's score), a run choice's probability gap
/// over this. The policy is often all but sure (gaps near 1000), and the worst fight moves score
/// hundreds below the bot's: at 10 a sure disagreement ranks with a move 10% of a win worse.
const CHOICE_SCALE: i64 = 10;

/// A disagreement, as the review keeps it (the page saves these, to pick up where it left).
fn found_fight(game: &Game, at: usize, mine: &RunAction, gap: &FightGap, round: u32) -> Value {
    let (you, theirs) = (score_permille(gap.mine), score_permille(gap.theirs));
    json!({
        "at": at,
        "kind": "combat",
        "room": room_word(game),
        "floor": game.run().state().total_floor(),
        "round": round,
        "rank": theirs - you,
        "you": words(game, mine),
        "bot": words(game, &RunAction::Combat(gap.best.clone())),
        "score": { "you": you, "bot": theirs },
        "ahead": gap.ahead,
        "behind": gap.behind,
        "worlds": gap.worlds,
        "options": gap.options,
    })
}

fn found_choice(game: &Game, at: usize, mine: &RunAction, gap: &ChoiceGap) -> Value {
    json!({
        "at": at,
        "kind": decision_word(game),
        "room": room_word(game),
        "floor": game.run().state().total_floor(),
        "rank": (i64::from(gap.theirs) - i64::from(gap.mine)) / CHOICE_SCALE,
        "you": words(game, mine),
        "bot": words(game, &gap.best),
        "share": { "you": gap.mine, "bot": gap.theirs },
        "options": gap.options,
    })
}

/// An action in the words the page shows it in.
fn words(game: &Game, action: &RunAction) -> Value {
    shown(game, action, None)["label"].clone()
}

/// The kind of room the run is in: `boss`, `elite`, `fight` or `null`.
fn room_word(game: &Game) -> Value {
    match game.run().state().room {
        sts2_core::run::Room::Combat { room_type, .. } => match room_type {
            RoomType::Boss => json!("boss"),
            RoomType::Elite => json!("elite"),
            _ => json!("fight"),
        },
        _ => Value::Null,
    }
}

/// The disagreement at `game`'s decision, the player having taken action `index` there.
fn disagreement(bot: &Bot, game: &Game, index: u32) -> Option<Value> {
    let run = game.run();
    if game.is_over() {
        return None;
    }
    let mine = game.recordable_actions().into_iter().nth(index as usize)?;
    let mut legal = run.legal_actions();
    legal.retain(|a| run.accepts(a));
    if legal.len() < 2 || !legal.contains(&mine) || follow_up(run).as_ref() == Some(&mine) {
        return None;
    }
    let at = game.actions().len();
    let decision = run.pending_decision();
    if let RunDecision::Combat(combat) = &decision {
        let RunAction::Combat(action) = &mine else {
            return None;
        };
        let gap = bot.fight_gap(run.state(), combat, &legal, action)?;
        return Some(found_fight(game, at, &mine, &gap, combat.state().round));
    }
    let gap = bot.choice_gap(run.state(), &decision, &legal, &mine)?;
    Some(found_choice(game, at, &mine, &gap))
}

/// The most disagreeing of `found`, one a fight turn.
fn top_picks(found: &[Value]) -> Vec<Value> {
    let mut order: Vec<&Value> = found.iter().collect();
    order.sort_by_key(|f| (std::cmp::Reverse(f["rank"].as_i64()), f["at"].as_u64()));
    let mut seen = Vec::new();
    let mut picks = Vec::new();
    for f in order {
        if f["kind"] == "combat" {
            let turn = (f["floor"].as_u64(), f["round"].as_u64());
            if seen.contains(&turn) {
                continue;
            }
            seen.push(turn);
        }
        picks.push(f.clone());
        if picks.len() == PICKS {
            break;
        }
    }
    picks
}

/// A finished run under review.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Review {
    game: Game,
    actions: Vec<u32>,
    character: usize,
    seed: String,
    ascension: u8,
    modifiers: String,
    found: Vec<Value>,
    /// Disagreements seen so far, kept or not.
    seen: u32,
}

#[wasm_bindgen]
impl Review {
    /// The review of a run (the save's fields, as JSON where they are lists), played to action
    /// `at`, holding what was `found` before it (`[]` to begin).
    ///
    /// # Errors
    /// If the run cannot be replayed.
    #[wasm_bindgen(constructor)]
    #[allow(
        clippy::too_many_arguments,
        reason = "the save's fields, across the wasm boundary"
    )]
    pub fn new(
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &str,
        actions: &str,
        at: usize,
        found: &str,
        counted: u32,
    ) -> Result<Self, String> {
        let actions = parse_actions(actions)?;
        let chosen = parse_modifiers(modifiers)?;
        let at = at.min(actions.len());
        let game = Game::replay(character, seed, ascension, &chosen, &actions[..at])?;
        Ok(Self {
            game,
            actions,
            character,
            seed: seed.to_owned(),
            ascension,
            modifiers: modifiers.to_owned(),
            found: serde_json::from_str(found).map_err(|e| e.to_string())?,
            seen: counted,
        })
    }

    /// The actions to go through.
    #[must_use]
    #[allow(
        clippy::missing_const_for_fn,
        reason = "wasm_bindgen takes no const functions"
    )]
    pub fn total(&self) -> usize {
        self.actions.len()
    }

    /// The actions gone through.
    #[must_use]
    pub fn at(&self) -> usize {
        self.game.actions().len()
    }

    /// Looks at one more of the player's actions; `false` once there are none left.
    ///
    /// # Errors
    /// If the run stops replaying.
    pub fn step(&mut self) -> Result<bool, String> {
        let at = self.at();
        let Some(&index) = self.actions.get(at) else {
            return Ok(false);
        };
        if let Some(found) = disagreement(bot(), &self.game, index) {
            self.seen += 1;
            self.found.push(found);
            if self.found.len() > KEEP * 2 {
                self.found
                    .sort_by_key(|f| std::cmp::Reverse(f["rank"].as_i64()));
                self.found.truncate(KEEP);
            }
        }
        self.game.advance(&[index])?;
        Ok(self.at() < self.total())
    }

    /// What to save, to pick up from [`Review::at`] later: `{found, seen}`.
    #[must_use]
    pub fn state(&self) -> String {
        json!({ "found": self.found, "seen": self.seen }).to_string()
    }

    /// The review's end: `{picks, seen, outcome}`, each fight pick with what the search saw at the
    /// end of each move's turn (`ends: {you, bot}`: `win`, `hp_tenths`, `potions`) and the HP
    /// before it (`hp`).
    ///
    /// # Errors
    /// If the run cannot be replayed.
    pub fn result(&self) -> Result<String, String> {
        let mut picks = top_picks(&self.found);
        for pick in &mut picks {
            if pick["kind"] == "combat" {
                self.read_turn_ends(pick)?;
            }
        }
        Ok(
            json!({ "picks": picks, "seen": self.seen, "outcome": outcome(&self.game) })
                .to_string(),
        )
    }

    /// The board at a pick's action, as the page draws it: `{view, you, bot}` (the taps of each
    /// move).
    ///
    /// # Errors
    /// If the run cannot be replayed.
    pub fn board(&self, at: usize) -> Result<String, String> {
        let mut game = self.at_action(at)?;
        let bots = crate::choose(bot(), &game).map(|(action, _)| action);
        let you = self
            .actions
            .get(at)
            .and_then(|&i| game.recordable_actions().into_iter().nth(i as usize));
        let you = you.map_or(Value::Null, |a| shown(&game, &a, None));
        let theirs = bots.map_or(Value::Null, |a| shown(&game, &a, None));
        Ok(json!({ "view": view_of(&mut game), "you": you, "bot": theirs }).to_string())
    }
}

impl Review {
    fn at_action(&self, at: usize) -> Result<Game, String> {
        let modifiers = parse_modifiers(&self.modifiers)?;
        let at = at.min(self.actions.len());
        Game::replay(
            self.character,
            &self.seed,
            self.ascension,
            &modifiers,
            &self.actions[..at],
        )
    }

    fn read_turn_ends(&self, pick: &mut Value) -> Result<(), String> {
        let at = pick["at"]
            .as_u64()
            .map_or(0, |a| usize::try_from(a).unwrap_or(0));
        let game = self.at_action(at)?;
        let run = game.run();
        let mine = self
            .actions
            .get(at)
            .and_then(|&i| game.recordable_actions().into_iter().nth(i as usize));
        let (decision, Some(RunAction::Combat(mine))) = (run.pending_decision(), mine) else {
            return Ok(());
        };
        let RunDecision::Combat(combat) = &decision else {
            return Ok(());
        };
        let mut legal = run.legal_actions();
        legal.retain(|a| run.accepts(a));
        let Some(gap) = bot().fight_gap(run.state(), combat, &legal, &mine) else {
            return Ok(());
        };
        let Some(ends) = bot().turn_ends(run.state(), combat, &legal, &[&mine, &gap.best]) else {
            return Ok(());
        };
        let read = |e: &sts2_diff::honest::search::TurnEnd| json!({ "win": e.win, "hp_tenths": e.hp_tenths, "potions": e.potions });
        pick["ends"] = json!({ "you": read(&ends[0]), "bot": read(&ends[1]) });
        let state = combat.state();
        pick["hp"] = json!(state.creature(state.player.creature).hp);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choose;

    fn played(character: usize, seed: &str, steps: usize) -> Game {
        let mut game = Game::new(character, seed, 0, &[]).expect("run");
        for _ in 0..steps {
            let Some((action, _)) = choose(bot(), &game) else {
                break;
            };
            game.act(action);
        }
        game
    }

    fn reviewed(character: usize, seed: &str, actions: &[u32], stop_at: Option<usize>) -> Review {
        let list = serde_json::to_string(actions).expect("json");
        let mut r = Review::new(character, seed, 0, "[]", &list, 0, "[]", 0).expect("review");
        while stop_at.is_none_or(|n| r.at() < n) && r.step().expect("step") {}
        r
    }

    #[test]
    fn a_run_played_as_the_bot_has_nothing_to_review() {
        let game = played(0, "web-review-1", 60);
        let r = reviewed(0, "web-review-1", game.actions(), None);
        let v: Value = serde_json::from_str(&r.result().expect("result")).expect("json");
        assert_eq!(v["picks"], json!([]), "{v}");
    }

    /// The bot's run with one worse fight move at the first fight decision with a gap.
    fn with_a_worse_move(character: usize, seed: &str) -> (Vec<u32>, usize) {
        let mut game = played(character, seed, 0);
        for _ in 0..200 {
            let run = game.run();
            let mut legal = run.legal_actions();
            legal.retain(|a| run.accepts(a));
            let decision = run.pending_decision();
            if let RunDecision::Combat(combat) = &decision {
                let gap = legal.iter().find_map(|a| match a {
                    RunAction::Combat(m) => bot()
                        .fight_gap(run.state(), combat, &legal, m)
                        .filter(|g| g.theirs - g.mine > 50_000)
                        .map(|_| a.clone()),
                    _ => None,
                });
                if let Some(worse) = gap {
                    let index = game
                        .recordable_actions()
                        .iter()
                        .position(|a| *a == worse)
                        .expect("recordable");
                    let at = game.actions().len();
                    let mut actions = game.actions().to_vec();
                    actions.push(u32::try_from(index).expect("small"));
                    return (actions, at);
                }
            }
            drop(decision);
            let (action, _) = choose(bot(), &game).expect("a move");
            game.act(action);
        }
        panic!("no fight move scored below the bot's");
    }

    #[test]
    fn a_worse_move_is_picked_with_the_scores_of_the_search() {
        let (actions, at) = with_a_worse_move(1, "web-review-2");
        let r = reviewed(1, "web-review-2", &actions, None);
        let v: Value = serde_json::from_str(&r.result().expect("result")).expect("json");
        let pick = &v["picks"][0];
        assert_eq!(pick["at"], json!(at), "{v}");
        assert_eq!(pick["kind"], "combat");
        assert_ne!(pick["you"], pick["bot"]);
        assert!(
            pick["score"]["bot"].as_i64() > pick["score"]["you"].as_i64(),
            "{pick}"
        );
        assert_eq!(
            pick["rank"].as_i64().expect("rank"),
            pick["score"]["bot"].as_i64().expect("bot")
                - pick["score"]["you"].as_i64().expect("you")
        );
        assert!(pick["ends"]["bot"]["win"].as_u64().is_some(), "{pick}");
        // The pick's bot move is the move the bot makes there, and the board names both.
        let board: Value = serde_json::from_str(&r.board(at).expect("board")).expect("json");
        assert_eq!(board["bot"]["label"], pick["bot"]);
        assert_eq!(board["you"]["label"], pick["you"]);
        assert_eq!(board["view"]["fight"], json!(true));
    }

    #[test]
    fn a_review_picked_up_again_ends_the_same() {
        let (actions, _) = with_a_worse_move(1, "web-review-2");
        let whole = reviewed(1, "web-review-2", &actions, None);
        let half = reviewed(1, "web-review-2", &actions, Some(actions.len() / 2));
        let state: Value = serde_json::from_str(&half.state()).expect("json");
        let list = serde_json::to_string(&actions).expect("json");
        let mut again = Review::new(
            1,
            "web-review-2",
            0,
            "[]",
            &list,
            half.at(),
            &state["found"].to_string(),
            u32::try_from(state["seen"].as_u64().expect("seen")).expect("small"),
        )
        .expect("review");
        while again.step().expect("step") {}
        assert_eq!(whole.result(), again.result());
    }
}
