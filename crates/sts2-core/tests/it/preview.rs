//! The previews a front end shows equal what playing does: random combats (every character,
//! random decks, relics and potions) where, before every action, each legal card play is
//! checked against the play itself.
//!
//! - A single-target card's `Damage` text ([`CombatState::card_preview`] at its target) is the
//!   damage of its first hit on that target; its `Block` text is a Block the player gains.
//! - [`Combat::play_outcome`] is what the play then does, and is `None` whenever the play uses
//!   a random number, draws or prompts.
//! - [`CombatState::unplayable_reason`] is `None` exactly for the cards that can be played.
//!
//! `PREVIEW_COMBATS=500 cargo test --profile regress -p sts2-core --test it preview`.

use sts2_core::Decimal;
use sts2_core::combat::{CardId, CombatState, CreatureId, DeckCard, Encounter, Event};
use sts2_core::content::cards::{CardKind, PreviewMode, TargetType, Var, VarClass, pools};
use sts2_core::content::powers::PowerKind;
use sts2_core::content::relics::RelicKind;
use sts2_core::{Action, Character, Combat, CombatConfig, Status};

/// `xorshift64*`: the test's own RNG (not game logic).
struct Fuzz(u64);

impl Fuzz {
    const fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).expect("fits u64")).expect("below n")
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

/// The powers that change the HP their owner loses after Block (`ModifyHpLost*`), which a
/// card's damage text does not count.
const HP_LOSS_POWERS: [PowerKind; 6] = [
    PowerKind::HardenedShell,
    PowerKind::Buffer,
    PowerKind::Intangible,
    PowerKind::HardToKill,
    PowerKind::DieForYou,
    PowerKind::Slippery,
];

/// Actions per combat.
const MAX_ACTIONS: usize = 400;

fn config(fuzz: &mut Fuzz, i: usize, character: Character, encounter: Encounter) -> CombatConfig {
    let ascension = u8::try_from(fuzz.below(11)).expect("0..=10");
    let mut config = CombatConfig::new(character, &format!("PREVIEW{i}"), ascension)
        .expect("every starter card is ported")
        .with_encounter(encounter);
    config.record_events = true;
    let cards: Vec<CardKind> = pools::character(character)
        .into_iter()
        .chain(pools::colorless())
        .filter_map(pools::PoolCard::kind)
        .collect();
    for _ in 0..fuzz.below(16) {
        let kind = fuzz.pick(&cards);
        let mut card = DeckCard::new(kind);
        if fuzz.below(3) == 0 && kind.is_upgradable(0) {
            card.upgrade_level = 1;
        }
        config.deck.push(card);
    }
    let relics: Vec<RelicKind> = character
        .relic_pool()
        .iter()
        .filter_map(|&(id, _)| RelicKind::from_id(id))
        .collect();
    for _ in 0..fuzz.below(4) {
        let kind = fuzz.pick(&relics);
        if config.relics.iter().all(|r| r.kind != kind) {
            config.relics.push(kind.into());
        }
    }
    config
}

/// The card's text value of the var of `class`'s kind at `target`, truncated as the text
/// shows it.
fn text_value(
    state: &CombatState,
    card: CardId,
    target: Option<CreatureId>,
    pick: fn(VarClass) -> bool,
) -> Option<i32> {
    let classes = state.card(card).kind.vars();
    state
        .card_preview(card, target, PreviewMode::Normal)
        .into_iter()
        .find(|&(var, _)| classes.iter().any(|v| v.name == var && pick(v.class)))
        .map(|(_, value): (Var, Decimal)| value.to_int_truncating())
}

/// Checks every legal card play of `combat`; returns the plays it compared.
fn check_plays(combat: &Combat, failures: &mut Vec<String>) -> usize {
    let state = combat.state();
    let legal = combat.legal_actions();
    for &card in &state.player.hand {
        let playable = legal
            .iter()
            .any(|a| matches!(a, Action::PlayCard { card: c, .. } if *c == card));
        let reason = state.unplayable_reason(card);
        if playable && reason.is_some() {
            failures.push(format!(
                "{:?} is playable but has a reason {reason:?}",
                state.card(card).kind
            ));
        }
    }
    let mut compared = 0;
    for action in legal {
        let Action::PlayCard { card, target } = action else {
            continue;
        };
        let kind = state.card(card).kind;
        let outcome = combat.play_outcome(card, target);
        let mut played = combat.clone();
        let events_before = played.events().len();
        let status = played.apply(action).expect("legal");
        let events = &played.events()[events_before..];
        let Some(outcome) = outcome else {
            continue;
        };
        compared += 1;
        if status == Status::Decision {
            failures.push(format!("{kind:?}: an outcome for a play that prompts"));
        }
        for c in &outcome.creatures {
            let after = played.state().creature(c.creature);
            if (after.hp, after.block, !after.is_alive() && c.dies)
                != (c.hp_after, c.block_after, c.dies)
            {
                failures.push(format!("{kind:?}: outcome {c:?}, played {after:?}"));
            }
        }
        // A creature the outcome leaves out is unchanged.
        for (i, (b, a)) in state
            .creatures
            .iter()
            .zip(&played.state().creatures)
            .enumerate()
        {
            let id = CreatureId(u32::try_from(i).expect("fits"));
            if outcome.of(id).is_none() && (b.hp, b.block) != (a.hp, a.block) {
                failures.push(format!("{kind:?}: outcome misses creature {i}"));
            }
        }
        check_text(state, card, target, &played, events, failures);
    }
    compared
}

/// Compares a card's `Damage` and `Block` text at `target` with the play's `events`
/// (`played` is the fight after it).
fn check_text(
    state: &CombatState,
    card: CardId,
    target: Option<CreatureId>,
    played: &Combat,
    events: &[Event],
    failures: &mut Vec<String>,
) {
    let kind = state.card(card).kind;
    let me = state.player.creature;
    let single = state.target_type(card) == TargetType::AnyEnemy;
    // The text is the damage as the hit starts: a play that changes a power before its
    // first hit (paying stars with a Strength-on-spend power) or a target that changes the
    // HP it loses after Block (Slippery) is not compared.
    let untouched = !events
        .iter()
        .take_while(|e| !matches!(e, Event::Damage { .. }))
        .any(|e| matches!(e, Event::PowerChanged { .. }));
    if let (true, true, Some(t)) = (single, untouched, target)
        && !state
            .creature(t)
            .powers
            .iter()
            .any(|p| HP_LOSS_POWERS.contains(&p.kind))
        && let Some(damage) = text_value(state, card, target, |c| matches!(c, VarClass::Damage(_)))
        && let Some((blocked, hp_lost)) = events
            .iter()
            .skip_while(|e| !matches!(e, Event::Attack { attacker, .. } if *attacker == me))
            .find_map(|e| match *e {
                Event::Damage {
                    dealer: Some(d),
                    receiver,
                    blocked,
                    hp_lost,
                } if d == me && receiver == t => Some((blocked, hp_lost)),
                _ => None,
            })
    {
        let dealt = blocked + hp_lost;
        // A killing hit loses at most the HP left.
        let killed = !played.state().creature(t).is_alive() || events.contains(&Event::Died(t));
        if dealt != damage && !(killed && dealt < damage) {
            failures.push(format!(
                "{kind:?}: text says {damage} damage, the first hit dealt {dealt}"
            ));
        }
    }
    // Other Block can come first (a power the card's damage sets off), so the card's own
    // is any of the me's gains.
    let gains: Vec<i32> = events
        .iter()
        .filter_map(|e| match *e {
            Event::BlockGained { creature, amount } if creature == me => Some(amount),
            _ => None,
        })
        .collect();
    if let Some(block) = text_value(state, card, target, |c| matches!(c, VarClass::Block(_)))
        && block > 0
        && !gains.is_empty()
        && !gains.contains(&block)
    {
        failures.push(format!(
            "{kind:?}: text says {block} Block, the me gained {gains:?}"
        ));
    }
}

#[test]
fn previews_match_the_play() {
    let combats: usize = std::env::var("PREVIEW_COMBATS")
        .ok()
        .map_or(100, |n| n.parse().expect("PREVIEW_COMBATS is a number"));
    let encounters = Encounter::ALL;
    let mut failures = Vec::new();
    let mut compared = 0;
    for i in 0..combats {
        let character = Character::ALL[i % Character::ALL.len()];
        let encounter = encounters[(i * 7 / Character::ALL.len()) % encounters.len()];
        let mut fuzz = Fuzz(0xA076_1D64_78BD_642F ^ (u64::try_from(i).expect("fits u64") << 1));
        let mut combat = Combat::new(config(&mut fuzz, i, character, encounter));
        for _ in 0..MAX_ACTIONS {
            match combat.status() {
                Status::PlayerTurn => compared += check_plays(&combat, &mut failures),
                Status::Decision => {}
                _ => break,
            }
            let mut actions = combat.legal_actions();
            let end_turn = actions.iter().position(|a| matches!(a, Action::EndTurn));
            if let Some(end) = end_turn
                && actions.len() > 1
                && fuzz.below(4) != 0
            {
                actions.swap_remove(end);
            }
            let action = actions.swap_remove(fuzz.below(actions.len()));
            combat.apply(action).expect("a legal action is accepted");
        }
    }
    failures.sort();
    failures.dedup();
    assert!(compared > 0, "no play was compared");
    assert!(
        failures.is_empty(),
        "{} mismatches over {compared} plays:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
