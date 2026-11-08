//! A run's anatomy (RLB16): its replayed actions as JSON lines, for the loss analysis.
//!
//! One line per fight (its start state and the HP it left) and one per run choice outside
//! fights (the decision's kind, the room, and what it changed: cards, relics, potions, gold,
//! HP). Reads only a finished run's actions; nothing here plays.

use std::fmt::Write as _;

use serde_json::{Value, json};
use sts2_core::combat::Action;
use sts2_core::content::cards::CardType;
use sts2_core::run::{Room, RoomType, Run, RunAction, RunCard, RunDecision, RunState};

use crate::reach::Fixture;

/// `card` as `ID` or `ID+` (upgraded).
fn card_name(card: &RunCard) -> String {
    let plus = if card.upgrade_level == 0 { "" } else { "+" };
    format!("{}{plus}", card.id)
}

fn deck(s: &RunState) -> Vec<String> {
    s.deck.iter().map(card_name).collect()
}

fn relics(s: &RunState) -> Vec<&'static str> {
    s.relics.iter().map(|r| r.kind.id()).collect()
}

fn potions(s: &RunState) -> Vec<&'static str> {
    s.potions.iter().flatten().copied().collect()
}

fn room(s: &RunState) -> String {
    match s.room {
        Room::Combat {
            room_type,
            encounter,
        } => format!("{room_type:?}:{encounter}"),
        Room::Event { id } => format!("Event:{id}"),
        Room::Ancient { id } => format!("Ancient:{id}"),
        ref other => format!("{other:?}"),
    }
}

/// The items of `after` not in `before` (multiset difference).
fn added<T: PartialEq + Clone>(before: &[T], after: &[T]) -> Vec<T> {
    let mut left = before.to_vec();
    after
        .iter()
        .filter(|x| {
            left.iter().position(|y| y == *x).is_none_or(|i| {
                left.swap_remove(i);
                false
            })
        })
        .cloned()
        .collect()
}

/// The decision's variant name.
fn decision_name(decision: &RunDecision<'_>) -> String {
    let text = format!("{decision:?}");
    text.split(['(', ' ', '{']).next().unwrap_or("").to_owned()
}

/// A fight's record at its start; `hp_after` and `potions_after` are added when it ends.
fn fight_start(i: usize, s: &RunState) -> Value {
    let kind = match s.room {
        Room::Combat {
            room_type: RoomType::Boss,
            ..
        } => "boss",
        Room::Combat {
            room_type: RoomType::Elite,
            ..
        } => "elite",
        _ => "common",
    };
    json!({
        "t": "fight", "i": i, "kind": kind, "room": room(s), "act": s.act_index + 1,
        "floor": s.total_floor(), "hp": s.hp, "max_hp": s.max_hp, "gold": s.gold,
        "deck": deck(s), "relics": relics(s), "potions": potions(s),
        "removals": s.card_shop_removals_used, "potion_turns": [],
    })
}

/// The run before a choice outside a fight, to log what the choice changed.
struct Before {
    decision: String,
    room: String,
    deck: Vec<String>,
    relics: Vec<&'static str>,
    potions: Vec<&'static str>,
    gold: i32,
    hp: i32,
    max_hp: i32,
    /// In a shop: the card removal's price, when it is still for sale.
    removal: Option<i32>,
    removable_curses: usize,
}

impl Before {
    fn of(run: &Run, decision: &RunDecision<'_>) -> Self {
        let s = run.state();
        Self {
            decision: decision_name(decision),
            room: room(s),
            deck: deck(s),
            relics: relics(s),
            potions: potions(s),
            gold: s.gold,
            hp: s.hp,
            max_hp: s.max_hp,
            removal: match decision {
                RunDecision::Shop(shop) if run.legal_actions().contains(&RunAction::RemoveCard) => {
                    Some(sts2_core::run::shop::price(s, shop.removal_base_cost))
                }
                _ => None,
            },
            removable_curses: s
                .deck
                .iter()
                .filter(|c| {
                    c.is_removable() && c.kind().is_some_and(|k| k.card_type() == CardType::Curse)
                })
                .count(),
        }
    }

    /// The choice `action` (the `i`th) as a line, `s` the run after it.
    fn line(&self, i: usize, action: &RunAction, s: &RunState) -> Value {
        let (deck_now, potions_now) = (deck(s), potions(s));
        json!({
            "t": "choice", "i": i, "d": self.decision, "room": self.room,
            "a": format!("{action:?}"), "act": s.act_index + 1, "floor": s.total_floor(),
            "cards_in": added(&self.deck, &deck_now), "cards_out": added(&deck_now, &self.deck),
            "relics_in": added(&self.relics, &relics(s)),
            "potions_in": added(&self.potions, &potions_now),
            "potions_out": added(&potions_now, &self.potions),
            "gold": s.gold - self.gold, "hp": s.hp - self.hp, "max_hp": s.max_hp - self.max_hp,
            "gold_before": self.gold, "removable_curses": self.removable_curses,
            "removal": self.removal,
        })
    }
}

/// The run of `fixture`, one JSON line per fight and per choice outside fights.
///
/// # Errors
/// If the run cannot be created or an action does not apply.
pub fn run_log(fixture: &Fixture) -> Result<String, String> {
    let mut run = Run::new(
        fixture.character,
        &fixture.seed,
        fixture.ascension,
        sts2_core::run::UnlockState::all(),
        fixture.act1,
    )
    .map_err(|e| e.to_string())?
    .with_events();
    let mut out = String::new();
    let mut fight: Option<Value> = None;
    for (i, action) in fixture.actions.iter().enumerate() {
        let decision = run.pending_decision();
        if matches!(decision, RunDecision::Victory | RunDecision::GameOver) {
            break;
        }
        if let Some(combat) = run.combat() {
            let f = fight.get_or_insert_with(|| fight_start(i, run.state()));
            if matches!(action, RunAction::Combat(Action::UsePotion { .. }))
                && let Some(turns) = f["potion_turns"].as_array_mut()
            {
                turns.push(json!(combat.state().player.turn_number));
            }
        }
        let before = (run.combat().is_none() && !matches!(action, RunAction::Combat(_)))
            .then(|| Before::of(&run, &decision));
        run.apply(action.clone())
            .map_err(|e| format!("action {} ({action:?}): {e}", i + 1))?;
        let s = run.state();
        if run.combat().is_none()
            && let Some(mut f) = fight.take()
        {
            f["hp_after"] = json!(s.hp);
            f["potions_after"] = json!(potions(s));
            let _ = writeln!(out, "{f}");
        }
        if let Some(before) = before {
            let _ = writeln!(out, "{}", before.line(i, action, s));
        }
    }
    let s = run.state();
    let end = match run.pending_decision() {
        RunDecision::Victory => "victory",
        RunDecision::GameOver => "death",
        _ => "other",
    };
    if let Some(mut f) = fight {
        f["hp_after"] = json!(s.hp);
        let _ = writeln!(out, "{f}");
    }
    let line = json!({
        "t": "end", "end": end, "act": s.act_index + 1, "floor": s.total_floor(), "hp": s.hp,
        "max_hp": s.max_hp, "gold": s.gold, "deck": deck(s), "relics": relics(s),
        "potions": potions(s), "removals": s.card_shop_removals_used, "room": room(s),
    });
    let _ = writeln!(out, "{line}");
    Ok(out)
}
