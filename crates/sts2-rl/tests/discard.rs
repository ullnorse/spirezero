//! RLN32: the discard-potion actions are listed only with the `discard_potion` option, only on a
//! rewards screen with a potion or a shop with an affordable one while the belt is full, and
//! taking one lets the claim or the purchase happen.

use sts2_core::run::shop::price;
use sts2_core::run::{Reward, Run, RunAction, RunDecision};
use sts2_rl::action;
use sts2_rl::run_action::{DISCARD_POTION_BASE, POTION_SLOTS, PROCEED, RUN_ACTIONS};
use sts2_rl::seeds::SplitMix;
use sts2_rl::{CombatMode, RunConfig, RunEnvs, RunOptions, SeedSource};

fn config(discard_potion: bool) -> RunConfig {
    RunConfig {
        ascension: 10,
        combat: CombatMode::Greedy,
        options: RunOptions {
            discard_potion,
            ..RunOptions::E2E
        },
        ..RunConfig::default()
    }
}

fn belt_full(run: &Run) -> bool {
    !run.state().potions.contains(&None)
}

fn held(run: &Run) -> usize {
    run.state().potions.iter().flatten().count()
}

/// Whether the screen is one where discarding makes room: a potion reward or an affordable shop
/// potion.
fn makes_room(run: &Run) -> bool {
    match run.pending_decision() {
        RunDecision::Rewards(rewards) => rewards.iter().any(|r| matches!(r, Reward::Potion(_))),
        RunDecision::Shop(shop) => shop
            .potions
            .iter()
            .any(|p| p.id.is_some() && price(run.state(), p.base_cost) <= run.state().gold),
        _ => false,
    }
}

#[test]
fn discards_are_offered_only_where_they_make_room_and_then_the_claim_or_buy_works() {
    let n = 8;
    let mut envs = RunEnvs::new(
        n,
        SeedSource::Range {
            start: 32_000_000,
            end: 32_000_200,
            cycle: false,
        },
        config(true),
    );
    let mut rngs: Vec<SplitMix> = (0..n)
        .map(|i| SplitMix::new(envs.episode(i).map_or(0, |(_, s)| s)))
        .collect();
    let (mut offered, mut taken, mut unlocked_claims, mut unlocked_buys) = (0, 0, 0, 0);
    let mut shop_offers = 0;
    // The env and belt count before each taken discard.
    let mut before: Vec<Option<usize>> = vec![None; n];
    while envs.live() > 0 {
        let mut actions = vec![0_u16; n];
        for i in 0..n {
            let Some(run) = envs.run(i) else { continue };
            let mask = envs.mask(i);
            let bits = |a: usize| mask[a / 64] >> (a % 64) & 1 == 1;
            let discards: Vec<usize> = (0..POTION_SLOTS)
                .map(|s| DISCARD_POTION_BASE + s)
                .filter(|&a| bits(a))
                .collect();
            if let Some(held_before) = before[i].take() {
                // The step after a discard: the belt has room, and the claim or buy is listed.
                assert_eq!(held(run), held_before - 1, "env {i}");
                if matches!(run.pending_decision(), RunDecision::Rewards(_)) {
                    unlocked_claims += usize::from(envs.legal(i).iter().any(|a| {
                        matches!(a, RunAction::ClaimReward(r)
                        if matches!(
                            run.pending_decision(),
                            RunDecision::Rewards(rs) if matches!(rs[*r], Reward::Potion(_))
                        ))
                    }));
                } else if matches!(run.pending_decision(), RunDecision::Shop(_)) {
                    unlocked_buys += usize::from(
                        envs.legal(i)
                            .iter()
                            .any(|a| matches!(a, RunAction::BuyPotion(_))),
                    );
                }
            }
            if !discards.is_empty() {
                offered += 1;
                shop_offers += usize::from(matches!(run.pending_decision(), RunDecision::Shop(_)));
                assert!(belt_full(run), "env {i}: discard with room in the belt");
                assert!(makes_room(run), "env {i}: discard that makes no room");
                assert_eq!(discards.len(), held(run), "one per held potion");
                assert!(bits(PROCEED) || matches!(run.pending_decision(), RunDecision::Shop(_)));
            }
            let pick = if discards.is_empty() || rngs[i].below(2) == 0 {
                let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
                let nth = u32::try_from(rngs[i].below(count as usize)).expect("small");
                action::nth_set(mask, nth).expect("a set bit")
            } else {
                taken += 1;
                before[i] = Some(held(run));
                discards[0]
            };
            assert!(pick < RUN_ACTIONS);
            actions[i] = u16::try_from(pick).expect("small");
        }
        envs.step(&actions).expect("legal");
        for (i, rng) in rngs.iter_mut().enumerate() {
            if envs.done()[i] == 1 {
                before[i] = None;
                *rng = SplitMix::new(envs.episode(i).map_or(0, |(_, s)| s));
            }
        }
    }
    eprintln!(
        "offered {offered} (shop {shop_offers}), taken {taken}, claims unlocked \
         {unlocked_claims}, buys unlocked {unlocked_buys}"
    );
    assert!(offered > 0 && taken > 0, "no discard was offered or taken");
    assert!(unlocked_claims > 0, "no discard ever let a claim happen");
}

#[test]
fn with_the_option_off_no_discard_is_ever_listed() {
    let n = 8;
    let mut envs = RunEnvs::new(
        n,
        SeedSource::Range {
            start: 32_000_000,
            end: 32_000_040,
            cycle: false,
        },
        config(false),
    );
    let mut rngs: Vec<SplitMix> = (0..n)
        .map(|i| SplitMix::new(envs.episode(i).map_or(0, |(_, s)| s)))
        .collect();
    while envs.live() > 0 {
        let mut actions = vec![0_u16; n];
        for i in 0..n {
            let mask = envs.mask(i);
            for s in 0..POTION_SLOTS {
                let a = DISCARD_POTION_BASE + s;
                assert_eq!(mask[a / 64] >> (a % 64) & 1, 0, "env {i}");
            }
            let count: u32 = mask.iter().map(|w| w.count_ones()).sum();
            if count > 0 {
                let nth = u32::try_from(rngs[i].below(count as usize)).expect("small");
                let a = action::nth_set(mask, nth).expect("a set bit");
                actions[i] = u16::try_from(a).expect("small");
            }
        }
        envs.step(&actions).expect("legal");
        for (i, rng) in rngs.iter_mut().enumerate() {
            if envs.done()[i] == 1 {
                *rng = SplitMix::new(envs.episode(i).map_or(0, |(_, s)| s));
            }
        }
    }
}
