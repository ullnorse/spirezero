//! The honest bot in the browser (WEB10): the web client's "?" ([`Advisor`]), the bot playing
//! the player's seed ([`Watch`]) and where the player's choices differed from its own
//! ([`differ`]), and the review of a finished run ([`review`]).
//!
//! It is its own wasm, loaded on demand by the page's Web Worker (`www/bot-worker.js`): the
//! bot's nets (`include_bytes`, about 7 MB) stay out of the page's wasm. The fights are the
//! champion's search at [`PHONE`]'s worlds, the run choices the learned run policy; both read
//! only what the player sees (`docs/specs/honesty.md`, `tests/it/honest.rs` in `sts2-diff`).

use std::sync::OnceLock;

use serde_json::{Value, json};
use sts2_core::Action;
use sts2_core::combat::Decision;
use sts2_core::run::{Modifier, RunAction, RunDecision};
use sts2_diff::honest::Bot;
use sts2_diff::honest::advice::{PHONE, Why};
use sts2_web::{Game, parse_modifiers};
use wasm_bindgen::prelude::wasm_bindgen;

pub mod review;

/// A run is stopped after this many actions (as the bot's runs: `sts2_diff::honest::MAX_ACTIONS`).
const MAX_ACTIONS: usize = sts2_diff::honest::MAX_ACTIONS;

/// The bot, built once (its nets take a moment to parse).
#[must_use]
pub fn bot() -> &'static Bot {
    static BOT: OnceLock<Bot> = OnceLock::new();
    BOT.get_or_init(|| Bot::champion(PHONE))
}

/// The bot's action at `game`'s position and why; `None` when the run is over or no action is
/// accepted.
#[must_use]
pub fn choose(bot: &Bot, game: &Game) -> Option<(RunAction, Why)> {
    if game.is_over() {
        return None;
    }
    let run = game.run();
    let mut legal = run.legal_actions();
    legal.retain(|a| run.accepts(a));
    if legal.is_empty() {
        return None;
    }
    let (action, why) = bot.advise(run.state(), &run.pending_decision(), &legal);
    Some((one_pick(action, &legal), why))
}

/// A whole card selection at once (how the search answers) as the page makes it, one card at a
/// time: the first of its cards that can still be picked, else Done.
fn one_pick(action: RunAction, legal: &[RunAction]) -> RunAction {
    let RunAction::Combat(Action::Resolve(Decision::Cards(cards))) = &action else {
        return action;
    };
    let resolve = |d| RunAction::Combat(Action::Resolve(d));
    cards
        .iter()
        .map(|&c| resolve(Decision::Pick(c)))
        .chain([resolve(Decision::Done)])
        .find(|a| legal.contains(a))
        .unwrap_or(action)
}

/// What the page shows of an action: its taps (empty when none was found), its words, why, and
/// its `index` among the run's recordable actions (the page finds the taps from its own screen
/// with it: `Web::taps`).
fn shown(game: &Game, action: &RunAction, why: Option<Why>) -> Value {
    let mut v = game.taps_for(action).map_or_else(
        || json!({ "taps": [], "label": game.action_words(action), "aims": [], "potion": null }),
        |t| t.json(),
    );
    v["index"] = json!(game.recordable_actions().iter().position(|a| a == action));
    v["why"] = why.map_or(Value::Null, |w| Value::String(w.line()));
    v["fight"] = Value::Bool(matches!(
        game.run().pending_decision(),
        RunDecision::Combat(_)
    ));
    v
}

/// The game's view as the page draws it, without the action list (the save is not needed).
fn view_of(game: &mut Game) -> Value {
    let mut v: Value = serde_json::from_str(&game.view()).unwrap_or_default();
    if let Some(save) = v.get_mut("save").and_then(Value::as_object_mut) {
        save.remove("actions");
    }
    v
}

/// The run's end as the page lists it: `{end, floor, hp, max_hp}`.
fn outcome(game: &Game) -> Value {
    let s = game.run().state();
    let end = match game.run().pending_decision() {
        RunDecision::Victory => "victory",
        RunDecision::GameOver => "defeat",
        _ => "stopped",
    };
    json!({ "end": end, "floor": s.total_floor(), "hp": s.hp, "max_hp": s.max_hp })
}

/// The pending decision's word, as the page's view names it (`map`, `rewards`, `combat`, ...).
fn decision_word(game: &Game) -> Value {
    serde_json::from_str(&game.clone().view())
        .map(|v: Value| v["decision"].clone())
        .unwrap_or_default()
}

fn parse_actions(json: &str) -> Result<Vec<u32>, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Advice at a run's position, the run kept between calls so each ask replays only the actions
/// since the last.
#[wasm_bindgen]
#[derive(Debug, Default)]
pub struct Advisor {
    game: Option<Game>,
}

#[wasm_bindgen]
impl Advisor {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The bot's move in the run `character`/`seed`/`ascension`/`modifiers` (the save's JSON
    /// keys, `""` for none) after `actions` (the save's JSON array), as JSON: `{taps, label, aims, why, fight}` (`null` at the run's end).
    ///
    /// # Errors
    /// If the run cannot be replayed.
    pub fn advise(
        &mut self,
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &str,
        actions: &str,
    ) -> Result<String, String> {
        let actions = parse_actions(actions)?;
        let modifiers = parse_modifiers(modifiers)?;
        let game = self.sync(character, seed, ascension, &modifiers, &actions)?;
        Ok(choose(bot(), game)
            .map_or(Value::Null, |(action, why)| shown(game, &action, Some(why)))
            .to_string())
    }
}

impl Advisor {
    /// The kept run moved to `actions`: played on when it is a prefix of them, else replayed.
    fn sync(
        &mut self,
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &[Modifier],
        actions: &[u32],
    ) -> Result<&Game, String> {
        let keep = self.game.as_ref().is_some_and(|g| {
            g.start() == (character, seed, ascension)
                && g.modifiers() == modifiers
                && actions.starts_with(g.actions())
        });
        if !keep {
            self.game = None;
        }
        let game = match self.game.take() {
            Some(mut game) => {
                let done = game.actions().len();
                game.advance(&actions[done..])?;
                game
            }
            None => Game::replay(character, seed, ascension, modifiers, actions)?,
        };
        Ok(self.game.insert(game))
    }
}

/// The bot playing a seed, one action a step, with the first place it leaves the player's run.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Watch {
    game: Game,
    /// The player's actions on the same seed.
    mine: Vec<u32>,
    /// Whether the bot has left the player's path.
    split: bool,
}

#[wasm_bindgen]
impl Watch {
    /// The bot on `character`/`seed`/`ascension`/`modifiers` (JSON keys, `""` for none); `mine`
    /// is the player's action list (JSON).
    ///
    /// # Errors
    /// The run cannot start or `mine` is not a list.
    #[wasm_bindgen(constructor)]
    pub fn new(
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &str,
        mine: &str,
    ) -> Result<Self, String> {
        Ok(Self {
            game: Game::new(character, seed, ascension, &parse_modifiers(modifiers)?)?,
            mine: parse_actions(mine)?,
            split: false,
        })
    }

    /// The start, as a frame (see [`Watch::step`]).
    #[must_use]
    pub fn first(&mut self) -> String {
        json!({ "view": view_of(&mut self.game), "step": 0 }).to_string()
    }

    /// Plays one action; the frame as JSON: `{view, step, move: {label, why, ..}, split?,
    /// done?, outcome?}`. `split` is set once, where the bot first chose otherwise than the
    /// player in the same position: `{floor, you, bot}` (each `{label, ..}`).
    #[must_use]
    pub fn step(&mut self) -> String {
        let step = self.game.actions().len();
        let chosen = (step < MAX_ACTIONS)
            .then(|| choose(bot(), &self.game))
            .flatten();
        let Some((action, why)) = chosen else {
            return json!({
                "view": view_of(&mut self.game),
                "step": step,
                "done": true,
                "outcome": outcome(&self.game),
            })
            .to_string();
        };
        let shown_move = shown(&self.game, &action, Some(why));
        let mut split = Value::Null;
        if !self.split && self.mine.starts_with(self.game.actions()) {
            let theirs = self
                .mine
                .get(step)
                .and_then(|&i| self.game.recordable_actions().into_iter().nth(i as usize));
            if let Some(theirs) = theirs.filter(|t| *t != action) {
                self.split = true;
                split = json!({
                    "kind": decision_word(&self.game),
                    "floor": self.game.run().state().total_floor(),
                    "you": shown(&self.game, &theirs, None),
                    "bot": shown_move,
                });
            }
        }
        self.game.act(action);
        let mut frame =
            json!({ "view": view_of(&mut self.game), "step": step + 1, "move": shown_move });
        if !split.is_null() {
            frame["split"] = split;
        }
        if self.game.is_over() {
            frame["done"] = Value::Bool(true);
            frame["outcome"] = outcome(&self.game);
        }
        frame.to_string()
    }
}

/// Where the player's run choices differed from the bot's, and how the player's run ended.
///
/// The choices outside fights, at the same position, in run order, at most `limit`, as JSON:
/// `{differ: [{floor, kind, you, bot}], outcome}`.
///
/// # Errors
/// If the run cannot be replayed.
#[wasm_bindgen]
pub fn differ(
    character: usize,
    seed: &str,
    ascension: u8,
    modifiers: &str,
    actions: &str,
    limit: usize,
) -> Result<String, String> {
    let actions = parse_actions(actions)?;
    let mut game = Game::new(character, seed, ascension, &parse_modifiers(modifiers)?)?;
    let mut out = Vec::new();
    for &index in &actions {
        if out.len() < limit
            && let Some(found) = differs(bot(), &game, index)
        {
            out.push(found);
        }
        game.advance(&[index])?;
    }
    Ok(json!({ "differ": out, "outcome": outcome(&game) }).to_string())
}

/// The player's action `index` at `game`'s run decision (not a fight's, not one applied on its
/// own), against the bot's there, when they differ.
fn differs(bot: &Bot, game: &Game, index: u32) -> Option<Value> {
    let run = game.run();
    let decision = run.pending_decision();
    if matches!(decision, RunDecision::Combat(_)) || game.is_over() {
        return None;
    }
    let mine = game.recordable_actions().into_iter().nth(index as usize)?;
    let mut legal = run.legal_actions();
    legal.retain(|a| run.accepts(a));
    if legal.len() < 2
        || !legal.contains(&mine)
        || sts2_view::ui::follow_up(run).as_ref() == Some(&mine)
    {
        return None;
    }
    let (theirs, why) = bot.advise(run.state(), &decision, &legal);
    drop(decision);
    (theirs != mine).then(|| {
        json!({
            "floor": run.state().total_floor(),
            "kind": decision_word(game),
            "you": shown(game, &mine, None),
            "bot": shown(game, &theirs, Some(why)),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drives a run with the bot for `steps` actions.
    fn bot_game(character: usize, seed: &str, steps: usize) -> Game {
        let mut game = Game::new(character, seed, 0, &[]).expect("run");
        for _ in 0..steps {
            let Some((action, _)) = choose(bot(), &game) else {
                break;
            };
            game.act(action);
        }
        game
    }

    #[test]
    fn the_advice_is_the_bots_move_and_its_taps_make_it() {
        let game = bot_game(0, "web-bot", 12);
        let actions = serde_json::to_string(game.actions()).expect("json");
        let mut advisor = Advisor::new();
        let v: Value = serde_json::from_str(
            &advisor
                .advise(0, "web-bot", 0, "", &actions)
                .expect("advice"),
        )
        .expect("json");
        let (action, why) = choose(bot(), &game).expect("a move");
        assert_eq!(v["why"], why.line());
        let mut tapped = game.clone();
        for id in v["taps"].as_array().expect("taps") {
            let id = u32::try_from(id.as_u64().expect("id")).expect("u32");
            let input = sts2_view::model::Input::Tap(sts2_view::model::ButtonId(id));
            tapped.input(&input);
        }
        let first = tapped.actions().get(game.actions().len()).copied();
        let index = game.recordable_actions().iter().position(|a| *a == action);
        assert_eq!(first.map(|i| i as usize), index, "{v}");
    }

    #[test]
    fn the_advisor_plays_on_from_its_last_run() {
        let game = bot_game(4, "web-bot-2", 30);
        let all = game.actions();
        let mut advisor = Advisor::new();
        let half = serde_json::to_string(&all[..all.len() / 2]).expect("json");
        let _ = advisor
            .advise(4, "web-bot-2", 0, "", &half)
            .expect("advice");
        let whole = serde_json::to_string(all).expect("json");
        let played_on = advisor
            .advise(4, "web-bot-2", 0, "", &whole)
            .expect("advice");
        let fresh = Advisor::new()
            .advise(4, "web-bot-2", 0, "", &whole)
            .expect("advice");
        assert_eq!(played_on, fresh);
    }

    #[test]
    fn the_watch_is_the_bots_run_and_finds_the_split() {
        // The player plays as the bot, then at the first choice after six actions takes
        // another of the actions the run accepts.
        let mut mine = bot_game(1, "web-bot-3", 6);
        let list = loop {
            let (bots, _) = choose(bot(), &mine).expect("a move");
            let alt = mine
                .recordable_actions()
                .iter()
                .position(|a| *a != bots && mine.run().accepts(a));
            if let Some(alt) = alt {
                let mut list = mine.actions().to_vec();
                list.push(u32::try_from(alt).expect("u32"));
                break list;
            }
            mine.act(bots);
        };
        let n = list.len() - 1;
        let mut watch = Watch::new(
            1,
            "web-bot-3",
            0,
            "",
            &serde_json::to_string(&list).expect("json"),
        )
        .expect("watch");
        let mut split = None;
        while split.is_none() && watch.game.actions().len() <= n {
            let frame: Value = serde_json::from_str(&watch.step()).expect("json");
            split = frame.get("split").cloned();
        }
        let split = split.expect("the split");
        assert_ne!(split["you"]["label"], split["bot"]["label"], "{split}");
        assert_eq!(watch.game.actions()[..n], list[..n]);
    }

    #[test]
    fn a_run_like_the_bots_differs_nowhere() {
        let game = bot_game(0, "web-bot-4", 40);
        let actions = serde_json::to_string(game.actions()).expect("json");
        let v: Value =
            serde_json::from_str(&differ(0, "web-bot-4", 0, "", &actions, 10).expect("differ"))
                .expect("json");
        assert_eq!(v["differ"], json!([]));
    }

    #[test]
    fn the_advisor_replays_a_run_with_its_modifiers() {
        let keys = ["SPECIALIZED", "MIDAS", "FLIGHT"];
        let modifiers: Vec<Modifier> = keys
            .iter()
            .map(|k| Modifier::from_key(k).expect("modifier"))
            .collect();
        let mut game = Game::new(0, "web-bot-mods", 0, &modifiers).expect("run");
        for _ in 0..25 {
            let Some((action, _)) = choose(bot(), &game) else {
                break;
            };
            game.act(action);
        }
        let actions = serde_json::to_string(game.actions()).expect("json");
        let json = serde_json::to_string(&keys).expect("json");
        let v: Value = serde_json::from_str(
            &Advisor::new()
                .advise(0, "web-bot-mods", 0, &json, &actions)
                .expect("advice"),
        )
        .expect("json");
        let (_, why) = choose(bot(), &game).expect("a move");
        assert_eq!(v["why"], why.line());
    }
}
