//! The taps on the page that make a given action: how the bot's advice is shown (WEB10).
//!
//! The page's buttons are tried on copies of the game, one, two or three taps deep (a card,
//! then its target; a shop's section, then its item, then the same again to confirm), until one
//! applies the action.

use serde_json::{Value, json};
use sts2_core::Action;
use sts2_core::run::RunAction;
use sts2_view::model::Input;
use sts2_view::names;

use crate::Game;

/// Taps tried before giving up.
const BUDGET: usize = 400;
/// The most taps an action takes.
const DEPTH: usize = 3;

/// A button of the screen as the page draws it.
#[derive(Clone, Debug)]
struct Seen {
    id: u32,
    label: String,
    /// The block (an enemy, a pet) whose target this button is.
    aims: Option<u32>,
    /// Whether the button asks to be tapped again (its label is then the question).
    asks: bool,
}

/// The text of a rich line (`[{text, ..}, ..]`).
fn text_of(rich: &Value) -> String {
    rich.as_array()
        .map(|spans| {
            spans
                .iter()
                .filter_map(|s| s["text"].as_str())
                .collect::<String>()
        })
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// Every button in `v`: objects with an `id`, a `state` that is not `Disabled` and a `label`.
fn buttons(v: &Value, parent: Option<&Value>, out: &mut Vec<Seen>) {
    match v {
        Value::Object(map) => {
            if let (Some(id), Some(state), Some(label)) = (
                map.get("id").and_then(Value::as_u64),
                map.get("state"),
                map.get("label"),
            ) && state != "Disabled"
                && let Ok(id) = u32::try_from(id)
            {
                let label = map
                    .get("card")
                    .and_then(|c| c["name"].as_str())
                    .map_or_else(|| text_of(label), str::to_owned);
                let aims = parent
                    .filter(|p| p.get("target").is_some_and(|t| t["id"] == id))
                    .and_then(|p| p["id"].as_u64())
                    .and_then(|p| u32::try_from(p).ok());
                let label = match (aims, parent.and_then(|p| p["name"].as_str())) {
                    (Some(_), Some(name)) => name.to_owned(),
                    _ => label,
                };
                let asks = map.get("confirm").is_some_and(|c| !c.is_null()) && *state == "Picked";
                out.push(Seen {
                    id,
                    label,
                    aims,
                    asks,
                });
            }
            for child in map.values() {
                buttons(child, Some(v), out);
            }
        }
        Value::Array(items) => {
            for item in items {
                buttons(item, parent, out);
            }
        }
        _ => {}
    }
}

/// The game's buttons: the screen's and the open sheet's, each once.
fn seen(game: &Game) -> Vec<Seen> {
    let mut out = Vec::new();
    let screen = serde_json::to_value(game.screen()).unwrap_or_default();
    buttons(&screen, None, &mut out);
    if let Some(sheet) = game.sheet() {
        buttons(
            &serde_json::to_value(sheet).unwrap_or_default(),
            None,
            &mut out,
        );
    }
    let mut ids = std::collections::HashSet::new();
    out.retain(|b| ids.insert(b.id));
    out
}

/// The client state a tap can change, as text.
fn ui_key(game: &Game) -> String {
    format!("{:?}|{:?}", game.ui, game.tossing)
}

/// How the page makes an action: the buttons to tap, what they say and the creatures aimed at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Taps {
    pub taps: Vec<u32>,
    pub label: String,
    pub aims: Vec<u32>,
    /// The belt slot of a potion to drink (the page's own sheet, not a screen button).
    pub potion: Option<usize>,
}

impl Taps {
    /// As JSON: `{taps, label, aims, potion}`.
    #[must_use]
    pub fn json(&self) -> Value {
        json!({ "taps": self.taps, "label": self.label, "aims": self.aims, "potion": self.potion })
    }
}

impl Game {
    /// The taps that apply `action` (one of the run's legal actions) from the screen as it is;
    /// `None` when none does within a few taps (the label then names the action alone:
    /// [`Game::action_words`]).
    #[must_use]
    pub fn taps_for(&self, action: &RunAction) -> Option<Taps> {
        if let Some(taps) = self.potion_taps(action) {
            return Some(taps);
        }
        let index = self.recordable_actions().iter().position(|a| a == action)?;
        let index = u32::try_from(index).ok()?;
        let base = self.actions.len();
        let mut frontier = vec![(self.clone(), Vec::<Seen>::new())];
        let mut budget = BUDGET;
        for depth in 0..DEPTH {
            let mut next = Vec::new();
            for (game, path) in frontier {
                let key = ui_key(&game);
                // The last tap is a confirmation: the button tapped before, again.
                let again = path.last().map(|b| b.id).filter(|_| depth + 1 == DEPTH);
                for button in seen(&game) {
                    if again.is_some_and(|id| id != button.id) {
                        continue;
                    }
                    budget = budget.checked_sub(1)?;
                    let mut tried = game.clone();
                    tried.input(&Input::Tap(sts2_view::model::ButtonId(button.id)));
                    let mut path = path.clone();
                    path.push(button);
                    if tried.actions.len() > base {
                        if tried.actions[base] == index {
                            let mut taps = taps_of(&path);
                            if let Some(side) = self.side_of(action) {
                                taps.label = format!("{} ({side})", taps.label);
                            }
                            return Some(taps);
                        }
                        continue;
                    }
                    if tried.leave.is_none() && ui_key(&tried) != key {
                        next.push((tried, path));
                    }
                }
            }
            frontier = next;
        }
        None
    }

    /// Where a map node is among the rooms the run can go to next (`left`, `middle`, `right`, `2nd
    /// from the left`), when there is more than one.
    fn side_of(&self, action: &RunAction) -> Option<String> {
        let RunAction::ChooseNode(coord) = action else {
            return None;
        };
        let mut cols: Vec<_> = self
            .run
            .legal_actions()
            .iter()
            .filter_map(|a| match a {
                RunAction::ChooseNode(c) => Some(c.col),
                _ => None,
            })
            .collect();
        cols.sort_unstable();
        cols.dedup();
        let at = cols.iter().position(|&c| c == coord.col)?;
        Some(match (cols.len(), at) {
            (0 | 1, _) => return None,
            (_, 0) => "left".to_owned(),
            (n, i) if i + 1 == n => "right".to_owned(),
            (3, 1) => "middle".to_owned(),
            (_, i) => format!(
                "{} from the left",
                ["", "2nd", "3rd", "4th", "5th", "6th"][i.min(5)]
            ),
        })
    }

    /// A potion drunk: the page's belt slot (its own sheet takes it from there) and the enemy
    /// aimed at.
    fn potion_taps(&self, action: &RunAction) -> Option<Taps> {
        let (slot, target) = match action {
            RunAction::UsePotion(slot) => (*slot, None),
            RunAction::Combat(Action::UsePotion { slot, target }) => (*slot, *target),
            _ => return None,
        };
        let player = self.run.combat().map(|c| c.state().player.creature);
        Some(Taps {
            taps: Vec::new(),
            label: self.action_words(action),
            aims: target
                .filter(|t| Some(*t) != player)
                .map(|t| t.0)
                .into_iter()
                .collect(),
            potion: Some(slot),
        })
    }

    /// `action` in a few words, for an action no tap was found for.
    #[must_use]
    pub fn action_words(&self, action: &RunAction) -> String {
        let potion = |slot: usize| {
            self.run
                .potions()
                .get(slot)
                .copied()
                .flatten()
                .map_or_else(|| "a potion".to_owned(), names::potion)
        };
        match action {
            RunAction::UsePotion(slot) | RunAction::Combat(Action::UsePotion { slot, .. }) => {
                format!("Drink {}", potion(*slot))
            }
            RunAction::DiscardPotion(slot) => format!("Throw away {}", potion(*slot)),
            RunAction::Combat(Action::EndTurn) => "End Turn".to_owned(),
            RunAction::SkipReward(_) => "Skip this reward".to_owned(),
            other => format!("{other:?}"),
        }
    }
}

/// The taps of a path, its words and its aims. A button asking to be tapped again says nothing
/// more; a map row's or reward's glyph (`M  `, `◉ `) goes.
fn taps_of(path: &[Seen]) -> Taps {
    let mut words: Vec<&str> = Vec::new();
    for b in path {
        let word = words_of(&b.label);
        if !b.asks && !word.is_empty() && words.last() != Some(&word) {
            words.push(word);
        }
    }
    Taps {
        taps: path.iter().map(|b| b.id).collect(),
        label: words.join(" → "),
        aims: path.iter().filter_map(|b| b.aims).collect(),
        potion: None,
    }
}

/// A label without its leading glyph: a symbol (`◉ 20 gold`) or a map row's letter and two
/// spaces (`M  Monster`).
fn words_of(label: &str) -> &str {
    let mut chars = label.chars();
    let first = chars.next();
    let rest = chars.as_str();
    match first {
        Some(c) if !c.is_alphanumeric() => rest.trim_start(),
        Some(_) if rest.starts_with("  ") => rest.trim_start(),
        _ => label,
    }
}
