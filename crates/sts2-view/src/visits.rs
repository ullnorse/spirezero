//! The rooms of a run so far, kept by a client for the history sheet and the end screen.

use sts2_core::run::{RoomType, Run, RunDecision};

use crate::describe::Describer;
use crate::names;
use crate::rooms::Visit;

/// Notes the room `run` is in (once per floor) in `visits`, and keeps the last visit's HP and
/// gold up to date.
pub fn note(visits: &mut Vec<Visit>, run: &Run, describer: &Describer) {
    let state = run.state();
    let (act, floor) = (state.act_index + 1, state.history.len());
    let what = match run.pending_decision() {
        RunDecision::Ancient(room) => Some(names::title_case(room.kind.id())),
        RunDecision::Combat(combat) => {
            let state = combat.state();
            let mut enemies: Vec<(String, usize)> = Vec::new();
            for &e in &state.enemies {
                let name = names::creature(state, e);
                match enemies.iter_mut().find(|(n, _)| *n == name) {
                    Some((_, count)) => *count += 1,
                    None => enemies.push((name, 1)),
                }
            }
            let enemies: Vec<String> = enemies
                .into_iter()
                .map(|(n, count)| {
                    if count > 1 {
                        format!("{count}x {n}")
                    } else {
                        n
                    }
                })
                .collect();
            let kind = match run.state().room_type() {
                Some(RoomType::Elite) => "Elite",
                Some(RoomType::Boss) => "Boss",
                _ => "Fight",
            };
            Some(format!("{kind}: {}", enemies.join(", ")))
        }
        RunDecision::Event(room) => Some(format!(
            "Event: {}",
            describer
                .event_title(room.kind.id())
                .unwrap_or_else(|| names::title_case(room.kind.id()))
        )),
        RunDecision::Shop(_) => Some("Shop".to_owned()),
        RunDecision::RestSite(_) => Some("Rest site".to_owned()),
        RunDecision::Treasure(_) => Some("Treasure".to_owned()),
        _ => None,
    };
    let hp = current_hp(run);
    let (max_hp, gold) = (state.max_hp, state.gold);
    let known = visits
        .last()
        .is_some_and(|v| v.act == act && v.floor == floor);
    if !known && let Some(what) = what {
        visits.push(Visit {
            act,
            floor,
            what,
            hp_in: hp,
            hp_out: hp,
            max_hp,
            gold_out: gold,
        });
    } else if let Some(last) = visits.last_mut() {
        last.hp_out = hp;
        last.max_hp = max_hp;
        last.gold_out = gold;
    }
}

/// The player's HP: the fight's while one runs.
fn current_hp(run: &Run) -> i32 {
    run.combat().map_or_else(
        || run.state().hp,
        |c| c.state().creature(c.state().player.creature).hp.max(0),
    )
}
