//! Random starts and random actions through the C ABI, with the invariants checked after
//! every step (`docs/reviews/spire-mentor.md`, idea 4).
//!
//! The input picks one to three start records (character, an act and one of its ported
//! encounters, ascension, a deck of any ported cards with upgrades and the data a deck card
//! needs (Mad Science's type), relics, a belt, HP, max energy, gold and floor), a batch of one
//! to four envs over them and a seed, then for each step one action per env among its legal
//! ones, sometimes an illegal one instead. After every call:
//! no panic (any panic aborts the fuzzer, even one the ABI would catch), an illegal batch
//! returns `STS2_ERR_ILLEGAL` and changes no buffer, a legal one returns the number of ended
//! episodes, `done`/`won` agree with it, no mask bit lies past the space, every live env has
//! a legal action, and every fight satisfies `sts2_rl::check_invariants` (HP bounds, block,
//! energy, hand ≤ 10, pile conservation).
//!
//! One input in three drives the run env instead (`sts2_run_envs_*`, stage 9): a seed list, range
//! or master seed, a character (or one per seed), ascension 0-10, who plays the fights (the agent
//! mostly; the greedy or random resolver), one to four envs, then random legal run-space actions
//! (sometimes an illegal one). After every call: an illegal batch returns `STS2_ERR_ILLEGAL` and
//! changes no buffer, `done`/`won`/`end` agree with the return code, no mask bit lies past the
//! space, every env with a run in progress has a legal action, and every fight in progress
//! satisfies `check_invariants`.
//!
//! `cargo fuzz run ffi_step -- -max_total_time=600` from the repository root.
#![no_main]

use std::ffi::CStr;

use libfuzzer_sys::arbitrary::{Result, Unstructured};
use libfuzzer_sys::fuzz_target;
use sts2_core::Character;
use sts2_core::combat::Encounter;
use sts2_core::content::cards::CardKind;
use sts2_core::content::potions::PotionKind;
use sts2_core::content::relics::RelicKind;
use sts2_core::run::RunDecision;
use sts2_core::run::rooms::ALL_ACTS;
use sts2_ffi::{
    STS2_ERR_ILLEGAL, Sts2RunConfig, sts2_envs_done_ptr, sts2_envs_free, sts2_envs_mask_ptr,
    sts2_envs_new_from_corpus, sts2_envs_obs_ptr, sts2_envs_step, sts2_envs_won_ptr,
    sts2_last_error, sts2_run_envs_done_ptr, sts2_run_envs_end_ptr, sts2_run_envs_free,
    sts2_run_envs_mask_ptr, sts2_run_envs_new, sts2_run_envs_obs_ptr, sts2_run_envs_step,
    sts2_run_envs_won_ptr,
};
use sts2_rl::action::nth_set;
use sts2_rl::invariants::{check_mask_bits, check_run_mask_bits};
use sts2_rl::start::StartCard;
use sts2_rl::{
    ACTIONS, MASK_WORDS, OBS_BYTES, RUN_ACTIONS, RUN_MASK_WORDS, RUN_OBS_BYTES, StartRecord,
    check_invariants,
};

/// The act index of each of `ALL_ACTS` (Overgrowth and Underdocks are both the first act).
const ACT_INDEX: [i32; 4] = [0, 0, 1, 2];

/// Steps per input at most.
const MAX_STEPS: usize = 300;

fn last_error() -> String {
    // SAFETY: the library returns a NUL-terminated string owned by this thread.
    unsafe { CStr::from_ptr(sts2_last_error()) }
        .to_string_lossy()
        .into_owned()
}

/// A game-shaped start record from the input.
fn record(u: &mut Unstructured<'_>) -> Result<StartRecord> {
    let character = *u.choose(&Character::ALL)?;
    let act = u.choose_index(ALL_ACTS.len())?;
    let ported: Vec<Encounter> = ALL_ACTS[act]
        .encounters
        .iter()
        .filter_map(|e| Encounter::from_entry(e.id))
        .collect();
    let encounter = match ported.as_slice() {
        [] => *u.choose(&Encounter::ALL)?,
        list => *u.choose(list)?,
    };
    let mut deck = Vec::new();
    for _ in 0..u.int_in_range(1..=40)? {
        let kind = *u.choose(&CardKind::ALL)?;
        let upgrade_level = u8::from(kind.is_upgradable(0) && u.ratio(1, 3)?);
        // A deck card's counter is 0 except for Mad Science, which enters a deck only through
        // Tinker Time with a type set: `TinkerTimeType` (Attack 1, Skill 2, Power 3) + 8 × rider.
        let counter = if kind == CardKind::MadScience {
            u.int_in_range(1..=3)?
        } else {
            0
        };
        deck.push(StartCard {
            kind,
            upgrade_level,
            enchantment: None,
            counter,
        });
    }
    let mut relics = Vec::new();
    for _ in 0..u.int_in_range(0..=6)? {
        let relic = *u.choose(&RelicKind::ALL)?;
        if !relics.contains(&relic) {
            relics.push(relic);
        }
    }
    let potion_slots = u.int_in_range(0..=5)?;
    let mut potions = Vec::new();
    for _ in 0..potion_slots {
        potions.push(if u.ratio(1, 2)? {
            Some(u.choose(&PotionKind::ALL)?.id().to_owned())
        } else {
            None
        });
    }
    let max_hp = u.int_in_range(1..=300)?;
    Ok(StartRecord {
        character,
        seed: format!("FZ{:X}", u.arbitrary::<u32>()?),
        ascension: u.int_in_range(0..=10)?,
        act_index: ACT_INDEX[act],
        encounter,
        deck,
        relics,
        potion_slots,
        potions,
        hp: u.int_in_range(1..=max_hp)?,
        max_hp,
        max_energy: u.int_in_range(1..=6)?,
        gold: u.int_in_range(0..=999)?,
        total_floor: u.int_in_range(1..=50)?,
        map_coord: None,
        relic_data: None,
        rng: None,
    })
}

/// The batch's buffers after a call: observations, masks, done, won.
struct Buffers<'a> {
    obs: &'a [u8],
    masks: &'a [u64],
    done: &'a [u8],
    won: &'a [u8],
}

/// # Safety
/// `envs` is a live batch of `n` envs.
unsafe fn buffers<'a>(envs: *const sts2_ffi::Sts2Envs, n: usize) -> Buffers<'a> {
    // SAFETY: the buffers have these sizes for the batch's life; nothing writes them while the
    // slices are used (they are rebuilt after every call).
    unsafe {
        Buffers {
            obs: std::slice::from_raw_parts(sts2_envs_obs_ptr(envs), n * OBS_BYTES),
            masks: std::slice::from_raw_parts(sts2_envs_mask_ptr(envs), n * MASK_WORDS),
            done: std::slice::from_raw_parts(sts2_envs_done_ptr(envs), n),
            won: std::slice::from_raw_parts(sts2_envs_won_ptr(envs), n),
        }
    }
}

/// A run batch's buffers after a call: observations, masks, done, won, end.
struct RunBuffers<'a> {
    obs: &'a [u8],
    masks: &'a [u64],
    done: &'a [u8],
    won: &'a [u8],
    end: &'a [u8],
}

/// # Safety
/// `envs` is a live run batch of `n` envs.
unsafe fn run_buffers<'a>(envs: *const sts2_ffi::Sts2RunEnvs, n: usize) -> RunBuffers<'a> {
    // SAFETY: the buffers have these sizes for the batch's life; nothing writes them while the
    // slices are used (they are rebuilt after every call).
    unsafe {
        RunBuffers {
            obs: std::slice::from_raw_parts(sts2_run_envs_obs_ptr(envs), n * RUN_OBS_BYTES),
            masks: std::slice::from_raw_parts(sts2_run_envs_mask_ptr(envs), n * RUN_MASK_WORDS),
            done: std::slice::from_raw_parts(sts2_run_envs_done_ptr(envs), n),
            won: std::slice::from_raw_parts(sts2_run_envs_won_ptr(envs), n),
            end: std::slice::from_raw_parts(sts2_run_envs_end_ptr(envs), n),
        }
    }
}

/// A run batch's configuration from the input; its seed list is in `seeds`.
fn run_config(u: &mut Unstructured<'_>, seeds: &mut Vec<u64>) -> Result<Sts2RunConfig> {
    let seed_kind = u.int_in_range(0..=2)?;
    for _ in 0..u.int_in_range(1..=4)? {
        seeds.push(u64::from(u.arbitrary::<u16>()?));
    }
    let start = u64::from(u.arbitrary::<u16>()?);
    Ok(Sts2RunConfig {
        seed_kind,
        cycle: u32::from(u.arbitrary::<bool>()?),
        seeds: seeds.as_ptr(),
        seeds_len: seeds.len(),
        start,
        end: start + u.int_in_range(1..=4)?,
        master: u.arbitrary()?,
        character: u.int_in_range(-1..=4)?,
        ascension: u.int_in_range(0..=10)?,
        // The agent mostly: the greedy resolver plans each turn on copies and is slow.
        combat: match u.int_in_range(0..=7)? {
            0 => 1,
            1 => 2,
            _ => 0,
        },
        intents: u32::from(u.arbitrary::<bool>()?),
    })
}

fn run_envs(u: &mut Unstructured<'_>) -> Result<()> {
    let mut seeds = Vec::new();
    let config = run_config(u, &mut seeds)?;
    let n = u.int_in_range(1..=4)?;
    // SAFETY: `config` and its seeds are readable.
    let envs = unsafe { sts2_run_envs_new(n, &raw const config, 1) };
    assert!(!envs.is_null(), "{}: {config:?}", last_error());
    let mut actions = vec![0_u16; n];
    let outcome = (|| -> Result<()> {
        for step in 0..MAX_STEPS {
            if u.is_empty() {
                break;
            }
            // SAFETY: `envs` is a live run batch of `n` envs.
            let b = unsafe { run_buffers(envs, n) };
            // SAFETY: `envs` is live and no call runs meanwhile.
            let handle = unsafe { &*envs };
            for (i, action) in actions.iter_mut().enumerate() {
                let mask: &[u64; RUN_MASK_WORDS] = b.masks[i * RUN_MASK_WORDS..][..RUN_MASK_WORDS]
                    .try_into()
                    .expect("words");
                let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
                if count == 0 {
                    // No episode left, `Run::new` failed, or the run ended at its reset (it is
                    // reported in this step).
                    let over = handle.envs().run(i).is_none_or(|run| {
                        matches!(
                            run.pending_decision(),
                            RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_)
                        )
                    });
                    assert!(
                        over || handle.envs().episode(i).is_none(),
                        "step {step}: run env {i} has no legal action: {config:?}"
                    );
                    *action = 0;
                    continue;
                }
                let nth = u.int_in_range(0..=count - 1)?;
                *action = u16::try_from(nth_set(mask, nth).expect("below count")).expect("small");
            }
            if u.ratio(1, 64)? {
                let env = u.choose_index(n)?;
                let mask = &b.masks[env * RUN_MASK_WORDS..][..RUN_MASK_WORDS];
                let live = mask.iter().any(|&w| w != 0);
                if live
                    && let Some(bad) = (0..RUN_ACTIONS).find(|&a| mask[a / 64] >> (a % 64) & 1 == 0)
                {
                    actions[env] = u16::try_from(bad).expect("small");
                    let before = (b.obs.to_vec(), b.masks.to_vec());
                    // SAFETY: `actions` has `n` aligned `u16`s.
                    let code = unsafe { sts2_run_envs_step(envs, actions.as_ptr(), n) };
                    assert_eq!(code, STS2_ERR_ILLEGAL, "{}", last_error());
                    // SAFETY: as above.
                    let after = unsafe { run_buffers(envs, n) };
                    assert!(
                        before.0 == after.obs && before.1 == after.masks,
                        "{config:?}"
                    );
                    continue;
                }
            }
            // SAFETY: `actions` has `n` aligned `u16`s.
            let code = unsafe { sts2_run_envs_step(envs, actions.as_ptr(), n) };
            assert!(
                code >= 0,
                "step {step}: code {code}: {}\n{config:?}",
                last_error()
            );
            // SAFETY: as above.
            let b = unsafe { run_buffers(envs, n) };
            let ended: i32 = b.done.iter().map(|&d| i32::from(d)).sum();
            assert_eq!(ended, code);
            for i in 0..n {
                assert!(b.done[i] <= 1 && b.won[i] <= b.done[i]);
                assert_eq!(b.done[i] == 1, b.end[i] != 0);
                assert!(b.end[i] <= 5);
                let mask: &[u64; RUN_MASK_WORDS] = b.masks[i * RUN_MASK_WORDS..][..RUN_MASK_WORDS]
                    .try_into()
                    .expect("words");
                check_run_mask_bits(mask).expect("no bit past the space");
            }
            // SAFETY: `envs` is live and no call runs meanwhile.
            let handle = unsafe { &*envs };
            for i in 0..n {
                if let Some(combat) = handle.envs().run(i).and_then(|r| r.combat())
                    && let Err(e) = check_invariants(combat)
                {
                    panic!("step {step}: run env {i}: {e}\n{config:?}");
                }
            }
        }
        Ok(())
    })();
    // SAFETY: `envs` is freed once.
    unsafe { sts2_run_envs_free(envs) };
    outcome
}

fn run(data: &[u8]) -> Result<()> {
    let mut u = Unstructured::new(data);
    if u.ratio(1, 3)? {
        return run_envs(&mut u);
    }
    let mut corpus = String::new();
    for _ in 0..u.int_in_range(1..=3)? {
        corpus.push_str(&record(&mut u)?.to_json());
        corpus.push('\n');
    }
    let n = u.int_in_range(1..=4)?;
    let seed = u.arbitrary::<u64>()?;
    // SAFETY: `corpus` has `corpus.len()` bytes.
    let envs = unsafe { sts2_envs_new_from_corpus(n, corpus.as_ptr(), corpus.len(), seed, 1) };
    if envs.is_null() {
        // Every record stopped at unported content before its first decision.
        let error = last_error();
        assert!(error.contains("no playable start"), "{error}\n{corpus}");
        return Ok(());
    }
    let mut actions = vec![0_u16; n];
    let outcome = (|| -> Result<()> {
        for step in 0..MAX_STEPS {
            if u.is_empty() {
                break;
            }
            // SAFETY: `envs` is a live batch of `n` envs.
            let b = unsafe { buffers(envs, n) };
            let illegal = u.ratio(1, 64)?;
            for (i, action) in actions.iter_mut().enumerate() {
                let mask: &[u64; MASK_WORDS] = b.masks[i * MASK_WORDS..][..MASK_WORDS]
                    .try_into()
                    .expect("words");
                let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
                assert!(
                    count > 0,
                    "step {step}: env {i} has no legal action\n{corpus}"
                );
                let nth = u.int_in_range(0..=count - 1)?;
                *action = u16::try_from(nth_set(mask, nth).expect("below count")).expect("small");
            }
            if illegal {
                let env = u.choose_index(n)?;
                let mask = &b.masks[env * MASK_WORDS..][..MASK_WORDS];
                if let Some(bad) = (0..ACTIONS).find(|&a| mask[a / 64] >> (a % 64) & 1 == 0) {
                    actions[env] = u16::try_from(bad).expect("small");
                    let before = (b.obs.to_vec(), b.masks.to_vec());
                    // SAFETY: `actions` has `n` aligned `u16`s.
                    let code = unsafe { sts2_envs_step(envs, actions.as_ptr(), n) };
                    assert_eq!(code, STS2_ERR_ILLEGAL, "{}", last_error());
                    // SAFETY: as above.
                    let after = unsafe { buffers(envs, n) };
                    assert!(before.0 == after.obs && before.1 == after.masks, "{corpus}");
                    continue;
                }
            }
            // SAFETY: `actions` has `n` aligned `u16`s.
            let code = unsafe { sts2_envs_step(envs, actions.as_ptr(), n) };
            assert!(
                code >= 0,
                "step {step}: code {code}: {}\n{corpus}",
                last_error()
            );
            // SAFETY: as above.
            let b = unsafe { buffers(envs, n) };
            let ended: i32 = b.done.iter().map(|&d| i32::from(d)).sum();
            assert_eq!(ended, code);
            assert!(b.done.iter().zip(b.won).all(|(&d, &w)| d <= 1 && w <= d));
            // SAFETY: `envs` is live and no call runs meanwhile.
            let handle = unsafe { &*envs };
            for i in 0..n {
                let mask: &[u64; MASK_WORDS] = b.masks[i * MASK_WORDS..][..MASK_WORDS]
                    .try_into()
                    .expect("words");
                check_mask_bits(mask).expect("no bit past the space");
                let combat = handle.envs().combat(i);
                if let Err(e) = check_invariants(combat) {
                    panic!("step {step}: env {i}: {e}\n{corpus}");
                }
            }
        }
        Ok(())
    })();
    // SAFETY: `envs` is freed once.
    unsafe { sts2_envs_free(envs) };
    outcome
}

fuzz_target!(|data: &[u8]| {
    let _ = run(data);
});
