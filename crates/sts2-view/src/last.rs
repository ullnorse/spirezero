//! The last-turn line.
//!
//! What the enemy turn, or your last play, just did, in one line under the incoming line, so
//! the fight reads without the log. Built from the fight's events after the
//! last `TurnStarted` (enemy turn) or `CardPlayed`/`PotionUsed` (your play); the HP figures
//! come from the events (HP lost, blocked) and the creatures' HP now, never worked out here.

use sts2_core::combat::{CombatState, CreatureId, Event, Side};

use crate::names;

/// The widest an enemy's name gets in the line.
const NAME_WIDTH: usize = 10;

/// `name` cut to `width` characters by dropping leading words (`Fuzzy Wurm Crawler` → `Crawler`).
///
/// Only while what is left still starts with a capital: `Eye with Teeth` stays whole rather
/// than read `with Teeth`. A name is never cut mid-word.
#[must_use]
pub fn short_name(name: &str, width: usize) -> String {
    let mut words: &str = name;
    while words.chars().count() > width {
        match words.split_once(' ') {
            Some((_, rest)) if rest.starts_with(|c: char| c.is_uppercase()) => words = rest,
            _ => break,
        }
    }
    if words.chars().count() <= width {
        words.to_owned()
    } else {
        name.to_owned()
    }
}

/// Per creature: the HP it lost and the damage its Block took, from one dealer or the turn.
#[derive(Default)]
struct Hurt {
    lost: i32,
    blocked: i32,
}

/// The index where the last thing that happened starts: the last card play, potion use or
/// enemy turn; the fight's first turn when nothing else happened yet.
fn start(events: &[Event]) -> Option<usize> {
    events
        .iter()
        .rposition(|e| {
            matches!(
                e,
                Event::CardPlayed { .. }
                    | Event::PotionUsed { .. }
                    | Event::TurnStarted {
                        side: Side::Enemy,
                        ..
                    }
            )
        })
        .or_else(|| {
            events
                .iter()
                .rposition(|e| matches!(e, Event::TurnStarted { .. }))
        })
}

/// `−7 HP (Slime 3, Twig 4) · 4 blocked`: what the player took in `events`, by dealer.
fn player_hurt(state: &CombatState, events: &[Event]) -> Vec<String> {
    let me = state.player.creature;
    let mut by_dealer: Vec<(Option<CreatureId>, i32)> = Vec::new();
    let mut total = Hurt::default();
    for e in events {
        if let Event::Damage {
            dealer,
            receiver,
            blocked,
            hp_lost,
        } = e
            && *receiver == me
        {
            total.lost += hp_lost;
            total.blocked += blocked;
            match by_dealer.iter_mut().find(|(d, _)| d == dealer) {
                Some(entry) => entry.1 += hp_lost,
                None => by_dealer.push((*dealer, *hp_lost)),
            }
        }
    }
    let mut parts = Vec::new();
    if total.lost > 0 {
        let dealers: Vec<String> = by_dealer
            .iter()
            .filter(|(_, lost)| *lost > 0)
            .map(|(dealer, lost)| {
                let who = dealer.map_or_else(
                    || "?".to_owned(),
                    |d| short_name(&names::creature(state, d), NAME_WIDTH),
                );
                format!("{who} {lost}")
            })
            .collect();
        if dealers.len() > 1 {
            parts.push(format!("−{} HP ({})", total.lost, dealers.join(", ")));
        } else {
            parts.push(format!("−{} HP", total.lost));
        }
    }
    if total.blocked > 0 {
        parts.push(format!("{} blocked", total.blocked));
    }
    parts
}

/// What the player's play did to each creature it hurt: `Leaf Slime −6 (8 → 2)`, `Twig ☠`;
/// without the name for `target` alone.
fn dealt(state: &CombatState, events: &[Event], target: Option<CreatureId>) -> Vec<String> {
    let me = state.player.creature;
    let mut hurt: Vec<(CreatureId, Hurt)> = Vec::new();
    for e in events {
        if let Event::Damage {
            dealer,
            receiver,
            blocked,
            hp_lost,
        } = e
            && *dealer == Some(me)
            && *receiver != me
        {
            let at = hurt
                .iter()
                .position(|(c, _)| c == receiver)
                .unwrap_or_else(|| {
                    hurt.push((*receiver, Hurt::default()));
                    hurt.len() - 1
                });
            hurt[at].1.lost += hp_lost;
            hurt[at].1.blocked += blocked;
        }
    }
    let died: Vec<CreatureId> = events
        .iter()
        .filter_map(|e| match e {
            Event::Died(c) => Some(*c),
            _ => None,
        })
        .collect();
    let mut parts = Vec::new();
    let alone = hurt.len() == 1;
    for (who, took) in &hurt {
        let mut what = Vec::new();
        if died.contains(who) {
            what.push("☠".to_owned());
        } else if took.lost > 0 {
            // The line is the latest thing that happened, so the HP now is the HP after.
            let after = state.creature(*who).hp.max(0);
            what.push(format!("−{} ({} → {after})", took.lost, after + took.lost));
        }
        if took.blocked > 0 {
            what.push(format!("{} blocked", took.blocked));
        }
        if what.is_empty() {
            continue;
        }
        let what = what.join(", ");
        if target == Some(*who) && alone {
            parts.push(what);
        } else {
            parts.push(format!(
                "{} {what}",
                short_name(&names::creature(state, *who), NAME_WIDTH)
            ));
        }
    }
    for who in died.iter().filter(|c| !hurt.iter().any(|(h, _)| h == *c)) {
        parts.push(format!(
            "{} ☠",
            short_name(&names::creature(state, *who), NAME_WIDTH)
        ));
    }
    parts
}

/// The rest of what happened in `events`, in a few words each: Block and powers gained or
/// lost (the player's without a name), deaths and escapes the caller did not cover, Osty
/// summoned, cards drawn.
fn other(state: &CombatState, events: &[Event], skip_deaths: bool) -> Vec<String> {
    let me = state.player.creature;
    let whose = |c: CreatureId| {
        if c == me {
            String::new()
        } else {
            format!("{} ", short_name(&names::creature(state, c), NAME_WIDTH))
        }
    };
    let mut parts = Vec::new();
    let mut drawn = 0;
    for e in events {
        match e {
            Event::BlockGained { creature, amount } if *creature == me && *amount > 0 => {
                parts.push(format!("+{amount} Block"));
            }
            Event::PowerChanged {
                creature,
                kind,
                amount,
            } if *amount != 0 => {
                parts.push(format!(
                    "{}{amount:+} {}",
                    whose(*creature),
                    names::power(*kind)
                ));
            }
            Event::Died(c) if !skip_deaths => parts.push(format!("{}☠", whose(*c))),
            Event::Escaped(c) => parts.push(format!("{}fled", whose(*c))),
            Event::Summoned { .. } => parts.push("Osty summoned".to_owned()),
            Event::CardDrawn(_) => drawn += 1,
            _ => {}
        }
    }
    if drawn > 0 {
        parts.push(format!("drew {drawn}"));
    }
    parts
}

/// The last thing that happened, in one line.
///
/// `Enemy turn: −7 HP (Slime 3, Twig 4) · +1 Frail · drew 5`, `Strike → Leaf Slime (S) −6
/// (8 → 2)`, `Bash → Twig Slime ☠`, `Defend: +5 Block`; `None` before anything happened (or
/// without the fight's events).
#[must_use]
pub fn line(state: &CombatState, events: &[Event]) -> Option<String> {
    let at = start(events)?;
    let rest = &events[at + 1..];
    let (head, target) = match &events[at] {
        Event::TurnStarted {
            side: Side::Enemy, ..
        } => {
            let mut parts = player_hurt(state, rest);
            parts.extend(other(state, rest, false));
            return Some(join("Enemy turn", &parts));
        }
        Event::TurnStarted { round, .. } => {
            return Some(join(&format!("Round {round}"), &other(state, rest, false)));
        }
        Event::CardPlayed { card, target } => (names::card(state, *card), *target),
        Event::PotionUsed { potion, target } => (potion.name().to_owned(), *target),
        _ => return None,
    };
    let mut parts = dealt(state, rest, target);
    parts.extend(other(state, rest, true));
    Some(target.map_or_else(
        || join(&head, &parts),
        |t| {
            let mut line = format!("{head} → {}", names::creature(state, t));
            // The target's own damage (unnamed) follows its name without a separator.
            let rest = match parts.split_first() {
                Some((first, more))
                    if (first.starts_with('−')
                        || first.starts_with('☠')
                        || first.ends_with("blocked"))
                        && !first.contains(" −") =>
                {
                    line.push(' ');
                    line.push_str(first);
                    more
                }
                _ => parts.as_slice(),
            };
            if !rest.is_empty() {
                line.push_str(" · ");
                line.push_str(&rest.join(" · "));
            }
            line
        },
    ))
}

/// `head: a · b`, or `head` alone.
fn join(head: &str, parts: &[String]) -> String {
    if parts.is_empty() {
        head.to_owned()
    } else {
        format!("{head}: {}", parts.join(" · "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_core::content::powers::PowerKind;
    use sts2_core::{Character, Combat, CombatConfig};

    fn fight() -> Combat {
        Combat::new(CombatConfig::new(Character::Ironclad, "UTWELVE", 0).expect("a fight"))
    }

    #[test]
    fn short_names_keep_a_whole_name() {
        assert_eq!(short_name("Fuzzy Wurm Crawler", 10), "Crawler");
        assert_eq!(short_name("Eye with Teeth", 10), "Eye with Teeth");
        assert_eq!(short_name("Leaf Slime (M)", 10), "Slime (M)");
        assert_eq!(short_name("Twig Slime (S)", 6), "Twig Slime (S)");
    }

    #[test]
    fn the_enemy_turn_is_summed_by_dealer() {
        let combat = fight();
        let state = combat.state();
        let (enemy, me) = (state.enemies[0], state.player.creature);
        let name = short_name(&names::creature(state, enemy), NAME_WIDTH);
        let events = [
            Event::TurnStarted {
                side: Side::Enemy,
                round: 1,
            },
            Event::MonsterMove {
                monster: enemy,
                name: "TACKLE",
            },
            Event::Damage {
                dealer: Some(enemy),
                receiver: me,
                blocked: 4,
                hp_lost: 3,
            },
            Event::PowerChanged {
                creature: me,
                kind: PowerKind::Frail,
                amount: 1,
            },
            Event::TurnStarted {
                side: Side::Player,
                round: 2,
            },
            Event::CardDrawn(state.player.hand[0]),
            Event::CardDrawn(state.player.hand[0]),
        ];
        assert_eq!(
            line(state, &events).as_deref(),
            Some("Enemy turn: −3 HP · 4 blocked · +1 Frail · drew 2")
        );
        let two = [
            events[0].clone(),
            events[2].clone(),
            Event::Damage {
                dealer: None,
                receiver: me,
                blocked: 0,
                hp_lost: 4,
            },
        ];
        assert_eq!(
            line(state, &two).as_deref(),
            Some(format!("Enemy turn: −7 HP ({name} 3, ? 4) · 4 blocked").as_str())
        );
    }

    #[test]
    fn a_play_says_what_it_did_to_its_target() {
        let combat = fight();
        let state = combat.state();
        let (enemy, me) = (state.enemies[0], state.player.creature);
        let card = state.player.hand[0];
        let name = names::creature(state, enemy);
        let hp = state.creature(enemy).hp;
        let hit = [
            Event::CardPlayed {
                card,
                target: Some(enemy),
            },
            Event::Damage {
                dealer: Some(me),
                receiver: enemy,
                blocked: 0,
                hp_lost: 6,
            },
        ];
        let card_name = names::card(state, card);
        assert_eq!(
            line(state, &hit).as_deref(),
            Some(format!("{card_name} → {name} −6 ({} → {hp})", hp + 6).as_str())
        );
        let kill = [hit[0].clone(), hit[1].clone(), Event::Died(enemy)];
        assert_eq!(
            line(state, &kill).as_deref(),
            Some(format!("{card_name} → {name} ☠").as_str())
        );
        let defend = [
            Event::CardPlayed { card, target: None },
            Event::BlockGained {
                creature: me,
                amount: 5,
            },
        ];
        assert_eq!(
            line(state, &defend).as_deref(),
            Some(format!("{card_name}: +5 Block").as_str())
        );
        assert_eq!(line(state, &[]), None);
    }
}
