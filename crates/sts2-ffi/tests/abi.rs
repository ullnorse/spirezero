//! The C ABI as a C caller sees it: build checks, the layout, random legal play through raw
//! pointers that never move, and every error path returning its code without a panic.
#![allow(
    clippy::multiple_unsafe_ops_per_block,
    reason = "a test block reads several of the library's pointers under one contract"
)]

use std::ffi::{CStr, CString};
use std::ptr;

use sts2_ffi::{
    STS2_ABI_VERSION, STS2_ERR_ILLEGAL, STS2_ERR_LENGTH, STS2_ERR_NULL, STS2_OK, Sts2Envs,
    Sts2Layout, Sts2RunConfig, Sts2RunEnvs, Sts2RunLayout, Sts2RunStepStats, Sts2StepStats,
    sts2_abi_version, sts2_build_id, sts2_envs_done_ptr, sts2_envs_free, sts2_envs_len,
    sts2_envs_mask_ptr, sts2_envs_new, sts2_envs_new_from_corpus, sts2_envs_obs_bytes,
    sts2_envs_obs_ptr, sts2_envs_reset_all, sts2_envs_static_ptr, sts2_envs_stats, sts2_envs_step,
    sts2_envs_won_ptr, sts2_last_error, sts2_layout, sts2_run_envs_done_ptr, sts2_run_envs_end_ptr,
    sts2_run_envs_floor_ptr, sts2_run_envs_free, sts2_run_envs_len, sts2_run_envs_mask_ptr,
    sts2_run_envs_new, sts2_run_envs_obs_ptr, sts2_run_envs_reset_all, sts2_run_envs_stats,
    sts2_run_envs_step, sts2_run_envs_won_ptr, sts2_run_layout, sts2_vocab_hash,
};
use sts2_rl::action::nth_set;
use sts2_rl::invariants::{check_mask_bits, check_run_mask_bits};
use sts2_rl::{
    ACTIONS, MASK_WORDS, OBS_BYTES, RUN_ACTIONS, RUN_MASK_WORDS, RUN_OBS_BYTES, STATIC_BYTES,
    check_invariants, vocab,
};

/// The calling thread's last error.
fn last_error() -> String {
    // SAFETY: the library returns a NUL-terminated string owned by this thread.
    unsafe { CStr::from_ptr(sts2_last_error()) }
        .to_string_lossy()
        .into_owned()
}

/// `SplitMix64`.
struct Rng(u64);

impl Rng {
    const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// The batch's buffer addresses.
fn pointers(envs: *const Sts2Envs) -> [usize; 5] {
    // SAFETY: `envs` is a live batch.
    unsafe {
        [
            sts2_envs_obs_ptr(envs).addr(),
            sts2_envs_static_ptr(envs).addr(),
            sts2_envs_mask_ptr(envs).addr(),
            sts2_envs_done_ptr(envs).addr(),
            sts2_envs_won_ptr(envs).addr(),
        ]
    }
}

/// Env `i`'s mask, read through the batch's pointer.
fn mask(envs: *const Sts2Envs, i: usize) -> [u64; MASK_WORDS] {
    // SAFETY: the mask buffer has `len × MASK_WORDS` words and `i` is below `len`.
    let words = unsafe {
        std::slice::from_raw_parts(sts2_envs_mask_ptr(envs), sts2_envs_len(envs) * MASK_WORDS)
    };
    words[i * MASK_WORDS..][..MASK_WORDS]
        .try_into()
        .expect("MASK_WORDS words")
}

#[test]
fn build_checks_and_layout() {
    assert_eq!(sts2_abi_version(), STS2_ABI_VERSION);
    // SAFETY: the build id is a static NUL-terminated string.
    let id = unsafe { CStr::from_ptr(sts2_build_id()) }
        .to_str()
        .expect("ASCII");
    assert_eq!(id.len(), 16);
    assert!(id.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(sts2_vocab_hash(), vocab::hash());
    let mut layout = Sts2Layout::default();
    // SAFETY: `layout` is writable.
    assert_eq!(unsafe { sts2_layout(&raw mut layout) }, STS2_OK);
    assert_eq!(layout.obs_bytes, OBS_BYTES as u64);
    assert_eq!(layout.static_bytes, STATIC_BYTES as u64);
    assert_eq!(layout.mask_words, MASK_WORDS as u64);
    assert_eq!(layout.actions, ACTIONS as u64);
    // SAFETY: null is allowed.
    assert_eq!(unsafe { sts2_layout(ptr::null_mut()) }, STS2_ERR_NULL);
}

#[test]
fn random_play_through_the_abi_keeps_the_buffers_and_the_invariants() {
    // SAFETY: a null path selects the default source.
    let envs = unsafe { sts2_envs_new(8, ptr::null(), 11, 2) };
    assert!(!envs.is_null(), "{}", last_error());
    // SAFETY: `envs` is live.
    let n = unsafe { sts2_envs_len(envs) };
    assert_eq!(n, 8);
    // SAFETY: as above.
    assert_eq!(unsafe { sts2_envs_obs_bytes(envs) }, OBS_BYTES);
    let at_start = pointers(envs);
    let mut rng = Rng(5);
    let mut actions = vec![0_u16; n];
    let mut ended = 0;
    for step in 0..3_000 {
        for (i, action) in actions.iter_mut().enumerate() {
            let bits = mask(envs, i);
            check_mask_bits(&bits).expect("no bit past the space");
            let count: u32 = bits.iter().map(|w| w.count_ones()).sum();
            assert!(count > 0, "a live env has a legal action");
            let nth = u32::try_from(rng.next() % u64::from(count)).expect("below count");
            *action = u16::try_from(nth_set(&bits, nth).expect("below count")).expect("small");
        }
        // SAFETY: `actions` has `n` aligned `u16`s.
        let code = unsafe { sts2_envs_step(envs, actions.as_ptr(), n) };
        assert!(code >= 0, "step {step}: {code} {}", last_error());
        ended += code;
        assert_eq!(pointers(envs), at_start, "the buffers moved");
        // SAFETY: `envs` is live; nothing writes it during the read.
        let handle = unsafe { &*envs };
        for i in 0..n {
            check_invariants(handle.envs().combat(i)).expect("the invariants hold");
        }
        // SAFETY: the done and won buffers have `n` bytes.
        let (done, won) = unsafe {
            (
                std::slice::from_raw_parts(sts2_envs_done_ptr(envs), n),
                std::slice::from_raw_parts(sts2_envs_won_ptr(envs), n),
            )
        };
        assert!(done.iter().zip(won).all(|(&d, &w)| w <= d && d <= 1));
        let reported: i32 = done.iter().map(|&d| i32::from(d)).sum();
        assert_eq!(reported, code);
    }
    assert!(ended > 50, "{ended} episodes ended");
    let mut stats = Sts2StepStats::default();
    // SAFETY: `envs` is live and `stats` writable.
    assert_eq!(unsafe { sts2_envs_stats(envs, &raw mut stats) }, STS2_OK);
    assert_eq!(stats.actions, 8);
    // SAFETY: `envs` is live.
    assert_eq!(unsafe { sts2_envs_reset_all(envs) }, STS2_OK);
    assert_eq!(pointers(envs), at_start, "the buffers moved at reset");
    // SAFETY: `envs` is freed once.
    unsafe { sts2_envs_free(envs) };
}

#[test]
fn errors_are_codes() {
    // SAFETY: a null path selects the default source.
    let envs = unsafe { sts2_envs_new(2, ptr::null(), 3, 1) };
    assert!(!envs.is_null());
    // An illegal action: the first index env 0's mask does not have.
    let bits = mask(envs, 0);
    let illegal = (0..ACTIONS)
        .find(|&a| bits[a / 64] >> (a % 64) & 1 == 0)
        .expect("some index is not legal");
    let legal_1 = (0..ACTIONS)
        .find(|&a| mask(envs, 1)[a / 64] >> (a % 64) & 1 == 1)
        .expect("a legal index");
    let actions = [
        u16::try_from(illegal).expect("small"),
        u16::try_from(legal_1).expect("small"),
    ];
    // SAFETY: the obs buffer has `2 × OBS_BYTES` bytes.
    let before =
        unsafe { std::slice::from_raw_parts(sts2_envs_obs_ptr(envs), 2 * OBS_BYTES) }.to_vec();
    // SAFETY: `actions` has 2 `u16`s.
    let code = unsafe { sts2_envs_step(envs, actions.as_ptr(), 2) };
    assert_eq!(code, STS2_ERR_ILLEGAL);
    assert!(
        last_error().contains("not legal in env 0"),
        "{}",
        last_error()
    );
    // SAFETY: as above.
    let after = unsafe { std::slice::from_raw_parts(sts2_envs_obs_ptr(envs), 2 * OBS_BYTES) };
    assert_eq!(before, after, "nothing was applied");
    // SAFETY: `actions` has at least 1 `u16`.
    let code = unsafe { sts2_envs_step(envs, actions.as_ptr(), 1) };
    assert_eq!(code, STS2_ERR_LENGTH);
    // SAFETY: null actions are refused.
    let code = unsafe { sts2_envs_step(envs, ptr::null(), 2) };
    assert_eq!(code, STS2_ERR_NULL);
    // SAFETY: a null batch is refused.
    let code = unsafe { sts2_envs_step(ptr::null_mut(), actions.as_ptr(), 2) };
    assert_eq!(code, STS2_ERR_NULL);
    // SAFETY: as above.
    let code = unsafe { sts2_envs_reset_all(ptr::null_mut()) };
    assert_eq!(code, STS2_ERR_NULL);
    // SAFETY: null reads give null or 0.
    unsafe {
        assert!(sts2_envs_obs_ptr(ptr::null()).is_null());
        assert_eq!(sts2_envs_len(ptr::null()), 0);
        sts2_envs_free(ptr::null_mut());
        sts2_envs_free(envs);
    }
}

#[test]
fn bad_sources_return_null_with_a_message() {
    // SAFETY: a null path selects the default source.
    assert!(unsafe { sts2_envs_new(0, ptr::null(), 0, 1) }.is_null());
    assert!(last_error().contains("at least one env"));
    let missing = CString::new("/nonexistent/starts.jsonl").expect("no NUL");
    // SAFETY: `missing` is NUL-terminated.
    assert!(unsafe { sts2_envs_new(1, missing.as_ptr(), 0, 1) }.is_null());
    assert!(last_error().contains("/nonexistent/starts.jsonl"));
    let junk = b"{\"character\": 3}\n";
    // SAFETY: `junk` has `junk.len()` bytes.
    assert!(unsafe { sts2_envs_new_from_corpus(1, junk.as_ptr(), junk.len(), 0, 1) }.is_null());
    assert!(last_error().starts_with("line 1:"), "{}", last_error());
    // SAFETY: an empty corpus may be null.
    assert!(unsafe { sts2_envs_new_from_corpus(1, ptr::null(), 0, 0, 1) }.is_null());
    assert!(last_error().contains("no playable start"));
}

#[test]
fn a_corpus_in_memory_builds_a_batch() {
    let line = r#"{"character":"Silent","seed":"SW3/Silent/A4/underdocks/F6","ascension":4,"act":0,"encounter":"SLIMES_WEAK","deck":[["STRIKE_SILENT",1,null,0],["DEFEND_SILENT",0,null,0],["NEUTRALIZE",0,null,0],["SURVIVOR",0,null,0]],"relics":["RING_OF_THE_SNAKE"],"potion_slots":2,"potions":[null,"FIRE_POTION"],"hp":51,"max_hp":70,"max_energy":3,"gold":120,"floor":6,"map_coord":[2,5]}"#;
    // SAFETY: `line` has `line.len()` bytes.
    let envs = unsafe { sts2_envs_new_from_corpus(3, line.as_ptr(), line.len(), 1, 1) };
    assert!(!envs.is_null(), "{}", last_error());
    // SAFETY: the obs buffer has `3 × OBS_BYTES` bytes; env 0's HP is the first `i16`.
    let obs = unsafe { std::slice::from_raw_parts(sts2_envs_obs_ptr(envs), 3 * OBS_BYTES) };
    assert_eq!(i16::from_le_bytes([obs[0], obs[1]]), 51);
    // SAFETY: `envs` is freed once.
    unsafe { sts2_envs_free(envs) };
}

/// A run batch's buffer addresses.
fn run_pointers(envs: *const Sts2RunEnvs) -> [usize; 6] {
    // SAFETY: `envs` is a live run batch.
    unsafe {
        [
            sts2_run_envs_obs_ptr(envs).addr(),
            sts2_run_envs_mask_ptr(envs).addr(),
            sts2_run_envs_done_ptr(envs).addr(),
            sts2_run_envs_won_ptr(envs).addr(),
            sts2_run_envs_end_ptr(envs).addr(),
            sts2_run_envs_floor_ptr(envs).addr(),
        ]
    }
}

/// A run config of the seeds `start..end` played by the agent.
const fn range_config(start: u64, end: u64, combat: u32) -> Sts2RunConfig {
    Sts2RunConfig {
        seed_kind: 1,
        cycle: 0,
        seeds: ptr::null(),
        seeds_len: 0,
        start,
        end,
        master: 0,
        character: -1,
        ascension: 0,
        combat,
        intents: 1,
    }
}

#[test]
fn random_run_play_through_the_abi_keeps_the_buffers_and_the_invariants() {
    let mut layout = Sts2RunLayout::default();
    // SAFETY: `layout` is writable.
    assert_eq!(unsafe { sts2_run_layout(&raw mut layout) }, STS2_OK);
    assert_eq!(layout.obs_bytes, RUN_OBS_BYTES as u64);
    assert_eq!(layout.mask_words, RUN_MASK_WORDS as u64);
    assert_eq!(layout.run_base, ACTIONS as u64);
    let config = Sts2RunConfig {
        seed_kind: 2,
        master: 3,
        ..range_config(0, 0, 0)
    };
    // SAFETY: `config` is readable.
    let envs = unsafe { sts2_run_envs_new(6, &raw const config, 2) };
    assert!(!envs.is_null(), "{}", last_error());
    // SAFETY: `envs` is live.
    let n = unsafe { sts2_run_envs_len(envs) };
    let at_start = run_pointers(envs);
    let mut rng = Rng(9);
    let mut actions = vec![0_u16; n];
    let mut ended = 0;
    for step in 0..2_000 {
        // SAFETY: the mask buffer has `n × RUN_MASK_WORDS` words; nothing writes it meanwhile.
        let masks =
            unsafe { std::slice::from_raw_parts(sts2_run_envs_mask_ptr(envs), n * RUN_MASK_WORDS) };
        for (action, bits) in actions
            .iter_mut()
            .zip(masks.as_chunks::<RUN_MASK_WORDS>().0)
        {
            check_run_mask_bits(bits).expect("no bit past the space");
            let count: u32 = bits.iter().map(|w| w.count_ones()).sum();
            assert!(count > 0, "step {step}: a live env has a legal action");
            let nth = u32::try_from(rng.next() % u64::from(count)).expect("below count");
            *action = u16::try_from(nth_set(bits, nth).expect("below count")).expect("small");
        }
        // SAFETY: `actions` has `n` aligned `u16`s.
        let code = unsafe { sts2_run_envs_step(envs, actions.as_ptr(), n) };
        assert!(code >= 0, "step {step}: {code} {}", last_error());
        ended += code;
        assert_eq!(run_pointers(envs), at_start, "the buffers moved");
        // SAFETY: `envs` is live; nothing writes it during the read.
        let handle = unsafe { &*envs };
        for i in 0..n {
            if let Some(combat) = handle.envs().run(i).and_then(|r| r.combat()) {
                check_invariants(combat).expect("the invariants hold");
            }
        }
        // SAFETY: the done, won and end buffers have `n` bytes.
        let (done, won, end) = unsafe {
            (
                std::slice::from_raw_parts(sts2_run_envs_done_ptr(envs), n),
                std::slice::from_raw_parts(sts2_run_envs_won_ptr(envs), n),
                std::slice::from_raw_parts(sts2_run_envs_end_ptr(envs), n),
            )
        };
        for i in 0..n {
            assert!(won[i] <= done[i] && done[i] <= 1);
            assert_eq!(done[i] == 1, end[i] != 0);
        }
        let reported: i32 = done.iter().map(|&d| i32::from(d)).sum();
        assert_eq!(reported, code);
    }
    assert!(ended >= 10, "{ended} runs ended");
    let mut stats = Sts2RunStepStats::default();
    // SAFETY: `envs` is live and `stats` writable.
    let code = unsafe { sts2_run_envs_stats(envs, &raw mut stats) };
    assert_eq!(code, STS2_OK);
    assert!(
        stats.decisions <= 6 && stats.actions >= stats.decisions,
        "{stats:?}"
    );
    // SAFETY: `envs` is live.
    assert_eq!(unsafe { sts2_run_envs_reset_all(envs) }, STS2_OK);
    assert_eq!(run_pointers(envs), at_start, "the buffers moved at reset");
    // SAFETY: `envs` is freed once.
    unsafe { sts2_run_envs_free(envs) };
}

#[test]
fn run_env_errors_are_codes() {
    // SAFETY: a null config is refused.
    assert!(unsafe { sts2_run_envs_new(1, ptr::null(), 1) }.is_null());
    assert!(last_error().contains("null"));
    for (config, message) in [
        (range_config(4, 4, 0), "no seed"),
        (range_config(0, 4, 3), "combat mode"),
        (
            Sts2RunConfig {
                character: 5,
                ..range_config(0, 4, 0)
            },
            "character",
        ),
        (
            Sts2RunConfig {
                ascension: 11,
                ..range_config(0, 4, 0)
            },
            "ascension",
        ),
        (
            Sts2RunConfig {
                seed_kind: 0,
                ..range_config(0, 4, 0)
            },
            "no seed",
        ),
    ] {
        // SAFETY: `config` is readable.
        assert!(unsafe { sts2_run_envs_new(2, &raw const config, 1) }.is_null());
        assert!(last_error().contains(message), "{}", last_error());
    }
    let seeds = [7_u64, 8];
    let config = Sts2RunConfig {
        seed_kind: 0,
        seeds: seeds.as_ptr(),
        seeds_len: seeds.len(),
        ..range_config(0, 0, 1)
    };
    // SAFETY: `config` and its seeds are readable.
    let envs = unsafe { sts2_run_envs_new(2, &raw const config, 1) };
    assert!(!envs.is_null(), "{}", last_error());
    // SAFETY: `envs` is live; the actions have the wrong length, then an illegal index.
    unsafe {
        assert_eq!(
            sts2_run_envs_step(envs, [0_u16].as_ptr(), 1),
            STS2_ERR_LENGTH
        );
        let masks = std::slice::from_raw_parts(sts2_run_envs_mask_ptr(envs), 2 * RUN_MASK_WORDS);
        let illegal = (0..RUN_ACTIONS)
            .find(|&a| masks[a / 64] >> (a % 64) & 1 == 0)
            .expect("an illegal index");
        let legal = nth_set(&masks[RUN_MASK_WORDS..], 0).expect("a legal index");
        let actions = [
            u16::try_from(illegal).expect("small"),
            u16::try_from(legal).expect("small"),
        ];
        assert_eq!(
            sts2_run_envs_step(envs, actions.as_ptr(), 2),
            STS2_ERR_ILLEGAL
        );
        assert_eq!(sts2_run_envs_step(envs, ptr::null(), 2), STS2_ERR_NULL);
        sts2_run_envs_free(envs);
    }
}
