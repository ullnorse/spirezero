//! `shuffle.json`: `UnstableShuffle`, `StableShuffle` and `List<T>.Sort()` tie order.

use std::cmp::Ordering;

use crate::common::{State, load};
use serde::Deserialize;
use sts2_core::Rng;
use sts2_core::shuffle::{introsort_by, stable_shuffle, stable_shuffle_by, unstable_shuffle};

#[derive(Deserialize)]
struct Fixture {
    int_lists: Vec<IntList>,
    tie_lists: Vec<TieList>,
}

#[derive(Deserialize)]
struct IntList {
    seed: u64,
    n: usize,
    scrambled_input: Vec<i32>,
    unstable_identity: Vec<i32>,
    stable_identity: Vec<i32>,
    stable_scrambled: Vec<i32>,
    #[serde(rename = "final")]
    final_state: State,
}

#[derive(Deserialize)]
struct TieList {
    pattern: String,
    n: usize,
    keys: Vec<i32>,
    sort_only: Vec<usize>,
    shuffles: Vec<TieShuffle>,
}

#[derive(Deserialize)]
struct TieShuffle {
    seed: u64,
    unstable: Vec<usize>,
    stable: Vec<usize>,
    #[serde(rename = "final")]
    final_state: State,
}

fn identity(n: usize) -> Vec<i32> {
    (0..i32::try_from(n).expect("small n")).collect()
}

#[test]
fn int_lists_match() {
    let fixture: Fixture = load("shuffle.json");
    for case in fixture.int_lists {
        let context = format!("seed {} n {}", case.seed, case.n);

        let mut scrambled = identity(case.n);
        Rng::new(12345).shuffle(&mut scrambled);
        assert_eq!(scrambled, case.scrambled_input, "{context}");

        let mut rng = Rng::new(case.seed);
        let mut list = identity(case.n);
        unstable_shuffle(&mut list, &mut rng);
        assert_eq!(list, case.unstable_identity, "{context}");
        case.final_state.assert_matches(&rng, &context);

        let mut list = identity(case.n);
        stable_shuffle(&mut list, &mut Rng::new(case.seed));
        assert_eq!(list, case.stable_identity, "{context}");

        let mut list = case.scrambled_input.clone();
        stable_shuffle(&mut list, &mut Rng::new(case.seed));
        assert_eq!(list, case.stable_scrambled, "{context}");
    }
}

#[test]
fn tie_lists_match() {
    let fixture: Fixture = load("shuffle.json");
    for case in fixture.tie_lists {
        assert_eq!(case.keys.len(), case.n);
        // Elements are tags (original indices) ordered by key only.
        let by_key = |a: &usize, b: &usize| -> Ordering { case.keys[*a].cmp(&case.keys[*b]) };
        let context = format!("{} n {}", case.pattern, case.n);

        let mut sorted: Vec<usize> = (0..case.n).collect();
        introsort_by(&mut sorted, by_key);
        assert_eq!(sorted, case.sort_only, "sort {context}");

        for shuffle in &case.shuffles {
            let context = format!("{context} seed {}", shuffle.seed);

            let mut list: Vec<usize> = (0..case.n).collect();
            unstable_shuffle(&mut list, &mut Rng::new(shuffle.seed));
            assert_eq!(list, shuffle.unstable, "unstable {context}");

            let mut rng = Rng::new(shuffle.seed);
            let mut list: Vec<usize> = (0..case.n).collect();
            stable_shuffle_by(&mut list, &mut rng, by_key);
            assert_eq!(list, shuffle.stable, "stable {context}");
            shuffle.final_state.assert_matches(&rng, &context);
        }
    }
}
