//! Combat feedback and practice undo, on the engine's own numbers.
//!
//! [`preview`] plays a card on a copy of the fight and reports the deltas (the engine's damage
//! path, so Strength, Vulnerable, Weak, Block, multi-hits, relics and powers all count); nothing
//! here works a number out. [`turn_note`] words what the enemies' turn did, from the events.
//! [`Undo`] keeps the start-of-card states of a practice run.

use serde_json::{Value, json};
use sts2_core::combat::state::PileType;
use sts2_core::combat::{
    Action, CardId, Combat, CreatureId, DecisionRequest, Event, IntentType, Orb, OrbId, OrbKind,
    Side, Status,
};
use sts2_core::content::cards::CardKind;
use sts2_core::content::powers::PowerKind;
use sts2_core::run::Run;

/// What playing `card` at `target` would do, as JSON; `None` when the play is not legal now.
///
/// `enemies`: per enemy the play changes, the `damage` dealt in all (Block taken plus HP lost),
/// the `blocked` part of it, the `hp_loss`, the `hits` that landed, and whether it `dies`.
/// `you`: the Block it gives and the HP it costs or heals. `incoming`: what the enemies' turn
/// would then cost, when the play is certain. `partial`: the play goes on to draw or roll,
/// which the numbers leave out.
#[must_use]
pub fn preview(combat: &Combat, card: CardId, target: Option<CreatureId>) -> Option<Value> {
    let outcome = combat.play_outcome_until_draw(card, target)?;
    let state = combat.state();
    let me = state.player.creature;
    let seen = combat.events().len();
    let mut after = combat.clone();
    let play = after.apply(Action::PlayCard { card, target });
    let events = after.events().get(seen..).unwrap_or_default();
    let hits = |who: CreatureId| {
        events
            .iter()
            .filter(|e| matches!(e, Event::Damage { receiver, .. } if *receiver == who))
            .count()
    };
    let enemies: Vec<Value> = outcome
        .creatures
        .iter()
        .filter(|c| c.creature != me)
        .map(|c| {
            let hp_loss = c.hp_before - c.hp_after;
            let blocked = (c.block_before - c.block_after).max(0);
            json!({
                "id": c.creature.0,
                "damage": hp_loss + blocked,
                "blocked": blocked,
                "hp_loss": hp_loss,
                "hits": hits(c.creature),
                "dies": c.dies,
                "block_gain": (c.block_after - c.block_before).max(0),
            })
        })
        .collect();
    let mine = outcome.of(me);
    let you = json!({
        "block_gain": mine.map_or(0, |c| (c.block_after - c.block_before).max(0)),
        "hp_delta": mine.map_or(0, |c| c.hp_after - c.hp_before),
    });
    let incoming = if outcome.partial || play.is_err() {
        None
    } else {
        after.incoming().map(|i| {
            json!({
                "total": i.total,
                "blocked": i.blocked,
                "absorbed": i.absorbed_by.iter().map(|&(_, n)| n).sum::<i32>(),
                "hp_loss": i.hp_loss,
                "hp_after": i.hp_after,
                "lethal": i.lethal,
                "certain": i.certain,
            })
        })
    };
    Some(json!({
        "card": card.0,
        "enemies": enemies,
        "you": you,
        "incoming": incoming,
        "wins": outcome.wins,
        "partial": outcome.partial,
    }))
}

/// `events` of `combat` as JSON, for playing them back as animations.
///
/// Creatures and cards go by the ids the view shows; a played card also names the pile it went
/// to (`to`: `Discard`, `Exhaust`, `Hand`, `Draw`, or `null` for a Power) once the input is done.
#[must_use]
pub fn events(combat: &Combat, events: &[Event]) -> Vec<Value> {
    let state = combat.state();
    events
        .iter()
        .map(|e| match e {
            Event::TurnStarted { side, round } => {
                json!({"kind": "TurnStarted", "side": format!("{side:?}"), "round": round})
            }
            Event::Shuffled => json!({"kind": "Shuffled"}),
            Event::CardDrawn(card) => json!({"kind": "CardDrawn", "card": card.0}),
            Event::CardPlayed { card, target } => json!({
                "kind": "CardPlayed",
                "card": card.0,
                "target": target.map(|t| t.0),
                "to": state.pile_of(*card).map(|p| format!("{p:?}")),
            }),
            Event::HandFlushed(cards) => json!({
                "kind": "HandFlushed",
                "cards": cards.iter().map(|c| c.0).collect::<Vec<_>>(),
            }),
            Event::PotionUsed { target, .. } => {
                json!({"kind": "PotionUsed", "target": target.map(|t| t.0)})
            }
            Event::MonsterMove { monster, name } => {
                json!({"kind": "MonsterMove", "monster": monster.0, "name": name})
            }
            Event::Attack {
                attacker,
                damage,
                hits,
            } => json!({
                "kind": "Attack",
                "attacker": attacker.0,
                "damage": damage.to_int_truncating(),
                "hits": hits,
            }),
            Event::Damage {
                dealer,
                receiver,
                blocked,
                hp_lost,
            } => json!({
                "kind": "Damage",
                "dealer": dealer.map(|d| d.0),
                "receiver": receiver.0,
                "blocked": blocked,
                "hp_lost": hp_lost,
            }),
            Event::BlockGained { creature, amount } => {
                json!({"kind": "BlockGained", "creature": creature.0, "amount": amount})
            }
            Event::PowerChanged {
                creature,
                kind,
                amount,
            } => json!({
                "kind": "PowerChanged",
                "creature": creature.0,
                "power": format!("{kind:?}"),
                "amount": amount,
            }),
            Event::Summoned { amount } => json!({"kind": "Summoned", "amount": amount}),
            Event::Died(c) => json!({"kind": "Died", "creature": c.0}),
            Event::Escaped(c) => json!({"kind": "Escaped", "creature": c.0}),
            Event::Won => json!({"kind": "Won"}),
            Event::Lost => json!({"kind": "Lost"}),
        })
        .collect()
}

/// The player's orbs before an input: each orb's id, kind and whether its passive would do
/// something (Glass at 0 does nothing).
#[must_use]
pub fn orbs_before(combat: &Combat) -> Vec<(OrbId, OrbKind, bool)> {
    let state = combat.state();
    state
        .player
        .orbs
        .iter()
        .map(|o| {
            (
                o.id,
                o.kind,
                state.orb_passive_val(*o).to_int_truncating() > 0,
            )
        })
        .collect()
}

/// GD41: the Sovereign Blades held, with their fields.
///
/// A change between an input's before and after is a Forge (`ForgeCmd.Forge`, which the engine has no event for).
/// The blade module is private to the core, so the fields are compared as their `Debug` text.
#[must_use]
pub fn blades(combat: &Combat) -> Vec<String> {
    let state = combat.state();
    PileType::LISTENER_ORDER
        .into_iter()
        .flat_map(|p| state.player.pile(p).iter().copied())
        .map(|c| state.card(c))
        .filter(|c| c.kind == CardKind::SovereignBlade)
        .map(|c| format!("{:?}", c.data))
        .collect()
}

/// GD41: a `Forge` event (`{"kind": "Forge", "new": no blade before}`, for `regent_forge` or `regent_refine`) after the card or potion that
/// forged, else after the last player turn start, when the blades changed.
pub fn forge_event(out: &mut Vec<Value>, before: &[String], after: &[String]) {
    if before == after {
        return;
    }
    let kind = |e: &Value, k: &str| e["kind"] == k;
    let at = out
        .iter()
        .position(|e| kind(e, "CardPlayed") || kind(e, "PotionUsed"))
        .or_else(|| {
            out.iter()
                .rposition(|e| kind(e, "TurnStarted") && e["side"] == "Player")
        })
        .map_or(0, |i| i + 1);
    out.insert(at, json!({"kind": "Forge", "new": before.is_empty()}));
}

/// Where each of the player's cards is before an input (hand, draw, discard, exhaust).
#[must_use]
pub fn piles_before(combat: &Combat) -> Vec<(CardId, PileType)> {
    let player = &combat.state().player;
    [
        PileType::Hand,
        PileType::Draw,
        PileType::Discard,
        PileType::Exhaust,
    ]
    .into_iter()
    .flat_map(|p| player.pile(p).iter().map(move |&c| (c, p)))
    .collect()
}

/// WEB67: the cards the input exhausted other than by playing them, as `Exhausted` events.
///
/// Brand's "Exhaust 1 card" and an Ethereal card at the end of the turn are such cards. Each is
/// `{"kind": "Exhausted", "card", "from"}` with the
/// pile it left (`null` for a card that came and went within the input). The engine has no
/// event for it, so the exhaust pile is compared with `before`. A played card that exhausts is
/// its `CardPlayed` (`to: "Exhaust"`). They go after the card or potion that caused them, else at
/// the hand's flush, else first.
pub fn exhaust_events(out: &mut Vec<Value>, before: &[(CardId, PileType)], combat: &Combat) {
    let played: Vec<u64> = out
        .iter()
        .filter(|e| e["kind"] == "CardPlayed")
        .filter_map(|e| e["card"].as_u64())
        .collect();
    let exhausted: Vec<Value> = combat
        .state()
        .player
        .pile(PileType::Exhaust)
        .iter()
        .filter(|c| !played.contains(&u64::from(c.0)))
        .filter_map(|&c| {
            let from = before.iter().find(|(b, _)| *b == c).map(|&(_, p)| p);
            (from != Some(PileType::Exhaust)).then(|| {
                json!({"kind": "Exhausted", "card": c.0, "from": from.map(|p| format!("{p:?}"))})
            })
        })
        .collect();
    if exhausted.is_empty() {
        return;
    }
    let kind = |e: &Value, k: &str| e["kind"] == k;
    let at = out
        .iter()
        .position(|e| kind(e, "CardPlayed") || kind(e, "PotionUsed"))
        .map(|i| i + 1)
        .or_else(|| out.iter().position(|e| kind(e, "HandFlushed")))
        .unwrap_or(0);
    out.splice(at..at, exhausted);
}

/// GD9: the input's orb changes as `Orb` events, which the engine does not have.
///
/// `{"kind": "Orb", "orb": "Lightning", "what": "channel"|"evoke"|"passive"}`, for the
/// frontend's sounds (`OrbModel.PlayChannelSfx` and its siblings). Evokes (orbs gone) and then
/// channels (new orbs) go after the card or potion that caused them, else after the last player
/// turn start, else first; ending the turn triggers the passives of the orbs held
/// (`BeforeTurnEndOrbTrigger`), before the enemies' turn.
pub fn orb_events(
    out: &mut Vec<Value>,
    before: &[(OrbId, OrbKind, bool)],
    after: &[Orb],
    ends: bool,
) {
    let orb = |kind: OrbKind, what: &str| json!({"kind": "Orb", "orb": format!("{kind:?}"), "what": what});
    let mut changed: Vec<Value> = before
        .iter()
        .filter(|(id, ..)| !after.iter().any(|o| o.id == *id))
        .map(|&(_, kind, _)| orb(kind, "evoke"))
        .collect();
    changed.extend(
        after
            .iter()
            .filter(|o| !before.iter().any(|(id, ..)| *id == o.id))
            .map(|o| orb(o.kind, "channel")),
    );
    if ends {
        let at = out
            .iter()
            .position(|e| e["kind"] == "TurnStarted" && e["side"] == "Enemy")
            .unwrap_or(out.len());
        let passives = before
            .iter()
            .filter(|&&(_, kind, acts)| acts && kind != OrbKind::Plasma)
            .map(|&(_, kind, _)| orb(kind, "passive"));
        out.splice(at..at, passives);
    }
    let kind = |e: &Value, k: &str| e["kind"] == k;
    let at = out
        .iter()
        .position(|e| kind(e, "CardPlayed") || kind(e, "PotionUsed"))
        .or_else(|| {
            out.iter()
                .rposition(|e| kind(e, "TurnStarted") && e["side"] == "Player")
        })
        .map_or(0, |i| i + 1);
    out.splice(at..at, changed);
}

/// The enemies' turn in one line (`took 14 · poison 3 · gained 5 block`), from the events since
/// the player's hand was flushed. `poisoned`: the player had Poison when the turn ended.
#[must_use]
pub fn turn_note(combat: &Combat, poisoned: bool) -> String {
    let state = combat.state();
    let me = state.player.creature;
    let events = combat.events();
    let from = events
        .iter()
        .rposition(|e| matches!(e, Event::HandFlushed(_)))
        .or_else(|| {
            events.iter().rposition(|e| {
                matches!(
                    e,
                    Event::TurnStarted {
                        side: Side::Enemy,
                        ..
                    }
                )
            })
        })
        .unwrap_or(0);
    let (mut took, mut other, mut block) = (0, 0, 0);
    for event in &events[from..] {
        match *event {
            Event::Damage {
                dealer,
                receiver,
                hp_lost,
                ..
            } if receiver == me => {
                if dealer.is_some_and(|d| d != me) {
                    took += hp_lost;
                } else {
                    other += hp_lost;
                }
            }
            Event::BlockGained { creature, amount } if creature == me => block += amount,
            _ => {}
        }
    }
    let mut parts = Vec::new();
    parts.push(if took > 0 {
        format!("took {took}")
    } else {
        "took no damage".to_owned()
    });
    if other > 0 {
        parts.push(format!(
            "{} {other}",
            if poisoned { "poison" } else { "lost" }
        ));
    }
    if block > 0 {
        parts.push(format!("gained {block} block"));
    }
    parts.join(" · ")
}

/// Whether the player has Poison.
#[must_use]
pub fn poisoned(combat: &Combat) -> bool {
    let state = combat.state();
    state
        .creature(state.player.creature)
        .has_power(PowerKind::Poison)
}

/// Whether a play showed the player something new.
///
/// That is a draw or shuffle, a random number, a changed enemy intent, or a card choice that
/// offers cards from the hidden draw pile. `before` is the fight when the play started, `after`
/// now.
#[must_use]
pub fn revealed(before: &Combat, after: &Combat) -> bool {
    let (b, a) = (before.state(), after.state());
    let seen = before.events().len();
    let drew = after
        .events()
        .get(seen..)
        .unwrap_or_default()
        .iter()
        .any(|e| matches!(e, Event::CardDrawn(_) | Event::Shuffled));
    let kinds = |s: &sts2_core::combat::CombatState| -> Vec<(CreatureId, Vec<IntentType>)> {
        s.enemies
            .iter()
            .filter(|&&e| s.creature(e).is_alive())
            .map(|&e| (e, s.intents(e).iter().map(|i| i.kind).collect()))
            .collect()
    };
    let hidden_options = match after.pending_decision() {
        Some(DecisionRequest::ChooseCards { options, .. }) => options
            .iter()
            .any(|c| !b.player.hand.contains(c) && !b.player.discard.contains(c)),
        None => false,
    };
    drew || b.rng != a.rng || kinds(b) != kinds(a) || hidden_options
}

/// A practice run's way back: the run as it stood before each card or potion of this turn,
/// while nothing has been revealed since.
#[derive(Clone, Debug, Default)]
pub struct Undo {
    steps: Vec<Step>,
}

#[derive(Clone, Debug)]
struct Step {
    run: Run,
    actions: usize,
    visits: usize,
}

impl Undo {
    /// Notes the run before a card or potion play.
    pub fn push(&mut self, run: &Run, actions: usize, visits: usize) {
        self.steps.push(Step {
            run: run.clone(),
            actions,
            visits,
        });
    }

    /// The fight as it stood before the last play noted.
    #[must_use]
    pub fn last_combat(&self) -> Option<&Combat> {
        self.steps.last()?.run.combat()
    }

    /// Forgets every step (end of turn, a reveal, the fight's end).
    pub fn clear(&mut self) {
        self.steps.clear();
    }

    /// Whether a play can be taken back.
    #[must_use]
    pub const fn available(&self) -> bool {
        !self.steps.is_empty()
    }

    /// The run, action count and visit count before the last play.
    pub fn pop(&mut self) -> Option<(Run, usize, usize)> {
        let step = self.steps.pop()?;
        Some((step.run, step.actions, step.visits))
    }
}

/// Whether `status` leaves the player free to act (no card choice open).
#[must_use]
pub fn settled(status: Status) -> bool {
    status != Status::Decision
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_core::Decimal;

    #[test]
    fn orb_events_follow_the_card_and_precede_the_enemy_turn() {
        let orb = |id, kind| Orb {
            id: OrbId(id),
            kind,
            value: Decimal::ZERO,
        };
        let before = [
            (OrbId(1), OrbKind::Lightning, true),
            (OrbId(2), OrbKind::Frost, true),
        ];
        let after = [orb(2, OrbKind::Frost), orb(3, OrbKind::Dark)];
        let mut out = vec![json!({"kind": "CardPlayed"}), json!({"kind": "Damage"})];
        orb_events(&mut out, &before, &after, false);
        let what: Vec<_> = out
            .iter()
            .map(|e| format!("{}{}", e["kind"], e["what"]))
            .collect();
        assert_eq!(
            what,
            [
                "\"CardPlayed\"null",
                "\"Orb\"\"evoke\"",
                "\"Orb\"\"channel\"",
                "\"Damage\"null"
            ]
        );
        let mut out = vec![json!({"kind": "TurnStarted", "side": "Enemy"})];
        orb_events(&mut out, &before[1..], &after[..1], true);
        assert_eq!(out[0]["what"], "passive");
        assert_eq!(out[0]["orb"], "Frost");
        assert_eq!(out.len(), 2);
    }
}
