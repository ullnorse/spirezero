//! Panic hunt: many combats with every character against every ported encounter, each with a
//! random deck (the starter deck plus cards of the character's and the colorless pool, some
//! upgraded), random relics and potions of the shared and the character's pools, played by a
//! random policy. Any panic fails the test, with the configurations that panicked.
//!
//! `FUZZ_COMBATS=20000 cargo test --profile regress -p sts2-core --test it fuzz_combats`
//! (`regress` is release with debug assertions); the default count keeps `cargo test` quick.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use sts2_core::combat::{DeckCard, Encounter};
use sts2_core::content::cards::{CardKind, pools};
use sts2_core::content::potions::PotionKind;
use sts2_core::content::relics::RelicKind;
use sts2_core::{Action, Character, Combat, CombatConfig, Status};

/// `xorshift64*`: the fuzzer's own RNG (not game logic).
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

/// Actions per combat before the fuzzer gives up on it (a stalemate is not a panic).
const MAX_ACTIONS: usize = 3000;

/// Whether `id` belongs to another character's pool in `pools`.
fn other_characters(
    character: Character,
    id: &str,
    pool: fn(Character) -> Vec<&'static str>,
) -> bool {
    Character::ALL
        .into_iter()
        .filter(|&c| c != character)
        .any(|c| pool(c).contains(&id))
}

fn relic_pool(c: Character) -> Vec<&'static str> {
    c.relic_pool().iter().map(|&(id, _)| id).collect()
}

fn potion_pool(c: Character) -> Vec<&'static str> {
    c.potion_pool().iter().map(|&(id, _)| id).collect()
}

/// A random combat for `character` against `encounter`.
fn config(fuzz: &mut Fuzz, i: usize, character: Character, encounter: Encounter) -> CombatConfig {
    let ascension = u8::try_from(fuzz.below(11)).expect("0..=10");
    let mut config = CombatConfig::new(character, &format!("FUZZ{i}"), ascension)
        .expect("every starter card is ported")
        .with_encounter(encounter);
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
    let relics: Vec<RelicKind> = RelicKind::ALL
        .into_iter()
        .filter(|r| !other_characters(character, r.id(), relic_pool))
        .collect();
    for _ in 0..fuzz.below(6) {
        let kind = fuzz.pick(&relics);
        if config.relics.iter().all(|r| r.kind != kind) {
            config.relics.push(kind.into());
        }
    }
    let potions: Vec<PotionKind> = PotionKind::ALL
        .into_iter()
        .filter(|p| !other_characters(character, p.id(), potion_pool))
        .collect();
    let belt: Vec<PotionKind> = (0..fuzz.below(config.potion_slots + 1))
        .map(|_| fuzz.pick(&potions))
        .collect();
    config.with_potions(&belt)
}

/// Plays `config` out with a random policy (ending the turn only one time in four when
/// something else is legal).
fn play(fuzz: &mut Fuzz, config: CombatConfig) -> String {
    let mut combat = Combat::new(config);
    for _ in 0..MAX_ACTIONS {
        if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
            return format!("{:?}", combat.status());
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
    "stalled".to_owned()
}

/// How the combats ended (`--nocapture` shows it): a fuzz that only meets unported content
/// tests little.
#[allow(
    clippy::print_stderr,
    reason = "the summary is for the person running the fuzz"
)]
fn report(ends: &std::collections::BTreeMap<String, usize>) {
    eprintln!("{ends:#?}");
}

#[test]
fn random_combats_never_panic() {
    // The panic hook keeps the last panic's location and message instead of printing it.
    static LAST: Mutex<String> = Mutex::new(String::new());
    let combats: usize = std::env::var("FUZZ_COMBATS")
        .ok()
        .map_or(200, |n| n.parse().expect("FUZZ_COMBATS is a number"));
    let encounters = Encounter::ALL;
    let mut failures = Vec::new();
    let mut ends = std::collections::BTreeMap::<String, usize>::new();
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|info| {
        *LAST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = info.to_string();
    }));
    for i in 0..combats {
        let character = Character::ALL[i % Character::ALL.len()];
        let encounter = encounters[(i / Character::ALL.len()) % encounters.len()];
        let mut fuzz = Fuzz(0x9E37_79B9_7F4A_7C15 ^ (u64::try_from(i).expect("fits u64") << 1));
        let result = catch_unwind(AssertUnwindSafe(|| {
            let config = config(&mut fuzz, i, character, encounter);
            play(&mut fuzz, config)
        }));
        if let Ok(end) = result {
            *ends.entry(end).or_insert(0) += 1;
        } else {
            let message = LAST
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .replace('\n', " ");
            failures.push(format!(
                "combat {i} ({character:?} vs {encounter:?}): {message}"
            ));
        }
    }
    std::panic::set_hook(hook);
    report(&ends);
    assert!(
        failures.is_empty(),
        "{} of {combats} combats panicked:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// `Combat::clone_from` (a search's scratch copy, which keeps its buffers) gives the same fight
/// as `clone`: random combats as above, where before each action one scratch fight, still
/// holding whatever it held last (another fight, another shape), is refilled from the fight and
/// compared with it, then plays a random legal action beside a fresh clone.
#[test]
fn clone_from_matches_clone() {
    let encounters = Encounter::ALL;
    let mut scratch: Option<Combat> = None;
    for i in 0..60 {
        let character = Character::ALL[i % Character::ALL.len()];
        let encounter = encounters[(i * 7) % encounters.len()];
        let mut fuzz = Fuzz(0x51ED_270B_2793_4C1D ^ (u64::try_from(i).expect("fits u64") << 1));
        let mut config = config(&mut fuzz, i, character, encounter);
        config.record_events = i % 2 == 0;
        let mut combat = Combat::new(config);
        let scratch = scratch.get_or_insert_with(|| combat.clone());
        for _ in 0..MAX_ACTIONS {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            let actions = combat.legal_actions();
            scratch.clone_from(&combat);
            assert_eq!(
                *scratch, combat,
                "combat {i}: clone_from differs from the source"
            );
            let probe = actions[fuzz.below(actions.len())].clone();
            let mut fresh = combat.clone();
            assert_eq!(scratch.apply(probe.clone()), fresh.apply(probe));
            assert_eq!(
                *scratch, fresh,
                "combat {i}: the refilled copy played differently"
            );
            let action = actions[fuzz.below(actions.len())].clone();
            combat.apply(action).expect("a legal action is accepted");
        }
    }
}
