//! `seed_hash.json`: XXH64, `SnakeCase`, `CanonicalizeSeed`, and every run/player stream seed.

use crate::common::{State, load};
use serde::Deserialize;
use sts2_core::rng::{
    PlayerRngSet, PlayerRngType, Rng, RunRngSet, RunRngType, canonicalize_seed, snake_case, xxh64,
};

#[derive(Deserialize)]
struct Fixture {
    xxh64: Vec<Hash>,
    snake_case: Vec<Snake>,
    canonicalize: Vec<Canonical>,
    seeds: Vec<RunCase>,
    named_rng: Vec<Named>,
}

#[derive(Deserialize)]
struct Hash {
    s: String,
    h: u64,
}

#[derive(Deserialize)]
struct Snake {
    s: String,
    snake: String,
}

#[derive(Deserialize)]
struct Canonical {
    s: String,
    canonical: String,
}

#[derive(Deserialize)]
struct RunCase {
    seed_string: String,
    seed: u64,
    streams: Vec<StreamCase>,
    players: Vec<PlayerCase>,
}

#[derive(Deserialize)]
struct StreamCase {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    stream_seed: u64,
    initial: State,
}

#[derive(Deserialize)]
struct PlayerCase {
    slot: u64,
    player_seed: u64,
    streams: Vec<StreamCase>,
}

#[derive(Deserialize)]
struct Named {
    seed: u64,
    name: String,
    initial: State,
}

fn fixture() -> Fixture {
    load("seed_hash.json")
}

#[test]
fn xxh64_matches() {
    let cases = fixture().xxh64;
    assert!(!cases.is_empty());
    for case in cases {
        assert_eq!(xxh64(&case.s), case.h, "xxh64({:?})", case.s);
    }
}

#[test]
fn snake_case_matches() {
    for case in fixture().snake_case {
        assert_eq!(snake_case(&case.s), case.snake, "snake_case({:?})", case.s);
    }
}

#[test]
fn canonicalize_seed_matches() {
    for case in fixture().canonicalize {
        assert_eq!(
            canonicalize_seed(&case.s),
            case.canonical,
            "canonicalize_seed({:?})",
            case.s
        );
    }
}

#[test]
fn run_rng_sets_match() {
    for seed in fixture().seeds {
        let set = RunRngSet::new(&seed.seed_string);
        assert_eq!(set.seed(), seed.seed, "{:?}", seed.seed_string);
        assert_eq!(set.seed_string(), seed.seed_string);
        assert_eq!(seed.streams.len(), RunRngType::ALL.len());
        for (kind, stream) in RunRngType::ALL.into_iter().zip(&seed.streams) {
            let context = format!("{:?} {kind:?}", seed.seed_string);
            assert_eq!(kind.enum_name(), stream.kind, "{context}");
            assert_eq!(kind.stream_name(), stream.name, "{context}");
            assert_eq!(
                seed.seed.wrapping_add(xxh64(&stream.name)),
                stream.stream_seed,
                "{context}"
            );
            stream.initial.assert_matches(&set[kind], &context);
        }
    }
}

#[test]
fn player_rng_sets_match() {
    for seed in fixture().seeds {
        for player in &seed.players {
            let set = PlayerRngSet::for_slot(seed.seed, player.slot);
            assert_eq!(set.seed(), player.player_seed);
            assert_eq!(player.streams.len(), PlayerRngType::ALL.len());
            for (kind, stream) in PlayerRngType::ALL.into_iter().zip(&player.streams) {
                let context = format!("{:?} slot {} {kind:?}", seed.seed_string, player.slot);
                assert_eq!(kind.enum_name(), stream.kind, "{context}");
                assert_eq!(kind.stream_name(), stream.name, "{context}");
                assert_eq!(
                    player.player_seed.wrapping_add(xxh64(&stream.name)),
                    stream.stream_seed,
                    "{context}"
                );
                stream.initial.assert_matches(&set[kind], &context);
            }
        }
    }
}

#[test]
fn named_rngs_match() {
    for case in fixture().named_rng {
        let rng = Rng::named(case.seed, &case.name);
        case.initial.assert_matches(&rng, &case.name);
    }
}
