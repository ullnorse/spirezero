//! The log (`l`): the whole enemy turn, one line per move with what it did; the start of the
//! turn and the plays.

use sts2_core::combat::{CombatState, CreatureId, Event};

use crate::names;
use crate::text::{Rich, RichSpan, Role};

/// The log's lines: one per event, except that an enemy move and its hits are one line
/// (`Twig Slime: Tackle 4 → 4 blocked`); what else happened follows on lines of its own.
fn log_lines(state: &CombatState, events: &[Event]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut i = 0;
    while i < events.len() {
        let Event::MonsterMove { monster, name } = &events[i] else {
            lines.extend(names::event_line(state, &events[i]));
            i += 1;
            continue;
        };
        i += 1;
        let start = i;
        while i < events.len()
            && !matches!(
                events[i],
                Event::MonsterMove { .. }
                    | Event::TurnStarted { .. }
                    | Event::CardPlayed { .. }
                    | Event::PotionUsed { .. }
                    | Event::Won
                    | Event::Lost
            )
        {
            i += 1;
        }
        lines.extend(move_summary(state, *monster, name, &events[start..i]));
    }
    lines
}

/// One enemy move and what followed it: a line with its hits (`5×3`) and where they went
/// (`→ 6 blocked, 9 damage`, per creature hit), then a line per other event.
fn move_summary(
    state: &CombatState,
    monster: CreatureId,
    name: &str,
    events: &[Event],
) -> Vec<String> {
    // Per creature hit: the hits' damage, blocked and HP lost.
    let mut hit: Vec<(CreatureId, Vec<i32>, i32, i32)> = Vec::new();
    let mut rest = Vec::new();
    for e in events {
        match *e {
            Event::Damage {
                dealer: Some(d),
                receiver,
                blocked,
                hp_lost,
            } if d == monster => {
                if !hit.iter().any(|h| h.0 == receiver) {
                    hit.push((receiver, Vec::new(), 0, 0));
                }
                let entry = hit
                    .iter_mut()
                    .find(|h| h.0 == receiver)
                    .expect("just added");
                entry.1.push(blocked + hp_lost);
                entry.2 += blocked;
                entry.3 += hp_lost;
            }
            Event::Attack { .. } => {}
            _ => rest.extend(names::event_line(state, e)),
        }
    }
    let mut parts = vec![format!(
        "{}: {}",
        names::creature(state, monster),
        names::monster_move(name)
    )];
    let amounts: Vec<i32> = hit.iter().flat_map(|h| h.1.iter().copied()).collect();
    if let Some(&first) = amounts.first() {
        parts.push(
            if amounts.len() > 1 && amounts.iter().all(|&a| a == first) {
                format!(" {first}×{}", amounts.len())
            } else {
                let each: Vec<String> = amounts.iter().map(ToString::to_string).collect();
                format!(" {}", each.join("+"))
            },
        );
        let results: Vec<String> = hit
            .iter()
            .map(|(who, _, blocked, lost)| {
                let mut what = Vec::new();
                if *blocked > 0 {
                    what.push(format!("{blocked} blocked"));
                }
                if *lost > 0 || *blocked == 0 {
                    what.push(format!("{lost} damage"));
                }
                let whom = if *who == state.player.creature {
                    String::new()
                } else {
                    format!("{}: ", names::creature(state, *who))
                };
                format!("{whom}{}", what.join(", "))
            })
            .collect();
        parts.push(format!(" → {}", results.join("; ")));
    }
    let mut lines = vec![parts.concat()];
    lines.extend(rest);
    lines
}

/// The log of `combat` as lines, the turns' headers titled.
#[must_use]
pub fn lines(combat: &sts2_core::Combat) -> Vec<Rich> {
    log_lines(combat.state(), combat.events())
        .into_iter()
        .map(|l| {
            let role = if l.starts_with("──") {
                Role::Title
            } else {
                Role::Plain
            };
            Rich(vec![RichSpan {
                fx: None,
                role,
                bold: false,
                text: l,
            }])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_core::{Character, Combat, CombatConfig, Decimal};

    fn fight(character: Character) -> Combat {
        Combat::new(CombatConfig::new(character, "UTWELVE", 0).expect("a fight"))
    }

    #[test]
    fn an_enemy_move_is_one_line_with_its_hits_and_effects() {
        let combat = fight(Character::Ironclad);
        let state = combat.state();
        let (enemy, me) = (state.enemies[0], state.player.creature);
        let name = names::creature(state, enemy);
        let hit = |blocked, hp_lost| Event::Damage {
            dealer: Some(enemy),
            receiver: me,
            blocked,
            hp_lost,
        };
        let events = [
            Event::MonsterMove {
                monster: enemy,
                name: "TACKLE_MOVE",
            },
            Event::Attack {
                attacker: enemy,
                damage: Decimal::from(4),
                hits: 3,
            },
            hit(4, 0),
            hit(2, 2),
            hit(0, 4),
            Event::BlockGained {
                creature: enemy,
                amount: 5,
            },
        ];
        assert_eq!(
            log_lines(state, &events),
            [
                format!("{name}: Tackle 4×3 → 6 blocked, 6 damage"),
                format!("{name} gains 5 block"),
            ]
        );
        let single = [events[0].clone(), hit(4, 0)];
        assert_eq!(
            log_lines(state, &single),
            [format!("{name}: Tackle 4 → 4 blocked")]
        );
    }
}
