//! The observation does not leak hidden state (`docs/reviews/spire-mentor.md`, idea 2): at
//! every position of random play on the RL bench's starts, reordering the draw pile and
//! reseeding every RNG stream (the run's streams and each monster's own) leaves the
//! observation's bytes unchanged. The same holds for the run env's observation at every
//! position of random runs, with the run's hidden state (its streams, odds, relic bags, the acts'
//! event and encounter lists, an event's own RNG) taken from another run. The v2 section (RLN16:
//! the honest forecast and the previews) passes the same checks, and so do RLN22's plan column and
//! RLN33's event section.

use sts2_core::combat::CombatState;
use sts2_core::rng::{Rng, RunRngSet};
use sts2_core::run::{Run, RunDecision, RunState};
use sts2_core::{Combat, Status};
use sts2_rl::obs::{CREATURES, OBS_BYTES, Observer};
use sts2_rl::obs_v2::{
    AIMED_AT, FORECAST_AT, PLAN_BYTES, SIDE_AT, STEP_BYTES, STEPS, V2_BYTES, V2Observer,
};
use sts2_rl::plan_column::{
    FULL_DEPTH, PLAN_SCORED, SHORT_DEPTH, big_fight, plan_column, plan_column_peeking, write_plan,
    write_plan_at,
};
use sts2_rl::priv_obs::{PRIV_BYTES, PrivObserver};
use sts2_rl::run_obs::{CombatView, RUN_OBS_BYTES, RunObserver, V2_AT};
use sts2_rl::{ACTIONS, RunConfig, RunMask, Start};

/// `SplitMix64`, the policy's RNG.
struct Policy(u64);

impl Policy {
    const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// `combat` with what the player cannot see changed ([`Combat::edit`]): the draw pile reversed
/// and rotated, every RNG stream reseeded from `salt`.
fn perturbed(combat: &Combat, salt: u64) -> Combat {
    let mut combat = combat.clone();
    combat.edit(|s| perturb(s, salt));
    combat
}

/// [`perturbed`]'s change to the state.
fn perturb(s: &mut CombatState, salt: u64) {
    let draw = &mut s.player.draw;
    draw.reverse();
    if draw.len() > 2 {
        let by = usize::try_from(salt).expect("a 64-bit usize") % draw.len();
        draw.rotate_left(by);
    }
    s.rng = RunRngSet::new(&format!("LEAK{salt}"));
    for (i, creature) in s.creatures.iter_mut().enumerate() {
        if let Some(monster) = &mut creature.monster {
            monster.rng = Rng::new(salt ^ (0x5EED << 16) ^ u64::try_from(i).expect("fits"));
        }
    }
}

/// The observation of `combat` with a fresh observer (so no cache carries a value over).
fn observe(combat: &Combat) -> Vec<u8> {
    let mut out = vec![0; OBS_BYTES];
    Observer::new(true).observe(combat, &mut out);
    out
}

#[test]
fn hidden_state_does_not_reach_the_observation() {
    let mut positions = 0;
    let mut shuffled = 0;
    let mut legal = Vec::new();
    for (seed, rep) in (0..110).flat_map(|seed| [(seed, 0), (seed, 1)]) {
        let mut combat = Start::w1_all(seed).combat;
        let mut policy = Policy(seed << 1 | rep);
        for step in 0..120 {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            let base = observe(&combat);
            for salt in [step, step ^ 0xABCD] {
                let other = perturbed(&combat, salt);
                shuffled +=
                    usize::from(other.state().player.draw[..] != combat.state().player.draw[..]);
                assert!(
                    base == observe(&other),
                    "seed {seed} rep {rep} step {step}: the observation changed with hidden state"
                );
            }
            positions += 1;
            combat.legal_actions_into(&mut legal);
            if legal.is_empty() {
                break;
            }
            let count = u64::try_from(legal.len()).expect("fits u64");
            let nth = usize::try_from(policy.next() % count).expect("below count");
            let action = legal.swap_remove(nth);
            combat.apply(action).expect("a listed action applies");
        }
    }
    assert!(positions > 4_000, "{positions} positions");
    assert!(shuffled > positions, "{shuffled} draw piles reordered");
}

/// `s` with what the player cannot see taken from `other` (another seed's run, played on): the
/// run's and the player's RNG streams, the card, potion and `?` odds, both relic bags, and each
/// act's event and encounter lists with their visit counts (the bosses and ancients are shown,
/// so they stay).
fn perturbed_run(s: &RunState, other: &RunState) -> RunState {
    let mut s = s.clone();
    s.rng.clone_from(&other.rng);
    s.player_rng.clone_from(&other.player_rng);
    s.card_rarity_odds = other.card_rarity_odds;
    s.potion_reward_odds = other.potion_reward_odds;
    s.unknown_odds = other.unknown_odds;
    s.shared_relic_bag.clone_from(&other.shared_relic_bag);
    s.player_relic_bag.clone_from(&other.player_relic_bag);
    for (mine, theirs) in s.acts.iter_mut().zip(&other.acts) {
        mine.events.clone_from(&theirs.events);
        mine.events_visited = theirs.events_visited;
        mine.normal_encounters.clone_from(&theirs.normal_encounters);
        mine.normal_encounters_visited = theirs.normal_encounters_visited;
        mine.elite_encounters.clone_from(&theirs.elite_encounters);
        mine.elite_encounters_visited = theirs.elite_encounters_visited;
        mine.boss_encounters_visited = theirs.boss_encounters_visited;
    }
    s
}

/// A run on `seed` as the run env starts it, played `steps` random legal actions on.
fn random_run(seed: u64, steps: u64) -> Run {
    let mut run = RunConfig::default().run(seed).expect("a run");
    let mut policy = Policy(seed ^ 0xD1CE);
    for _ in 0..steps {
        let legal = run.legal_actions();
        if legal.is_empty() {
            break;
        }
        let nth = usize::try_from(policy.next() % legal.len() as u64).expect("below len");
        let _ = run.apply(legal[nth].clone());
    }
    run
}

/// The run observation of the given parts with a fresh observer.
fn observe_run(
    s: &RunState,
    decision: &RunDecision<'_>,
    combat: Option<CombatView<'_>>,
) -> Vec<u8> {
    let mut out = vec![0; RUN_OBS_BYTES];
    RunObserver::new(true)
        .with_events(true)
        .observe_parts(s, decision, combat, &mut out);
    out
}

#[test]
fn hidden_run_state_does_not_reach_the_run_observation() {
    let mut positions = 0;
    let mut kinds = [0_u32; 14];
    for seed in 0..24_u64 {
        let mut run = RunConfig::default().run(seed).expect("a run");
        let mut policy = Policy(seed << 1 | 1);
        for step in 0..400_u64 {
            let decision = run.pending_decision();
            if matches!(
                decision,
                RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_)
            ) {
                break;
            }
            kinds[usize::from(sts2_rl::run_obs::phase_code(&decision))] += 1;
            let s = run.state();
            let view = run.combat().map(|combat| CombatView {
                combat,
                state: combat.state(),
            });
            let base = observe_run(s, &decision, view);
            let other = random_run(seed + 1_000 + step, step % 97);
            let hidden = perturbed_run(s, other.state());
            // An event's own `Rng` is hidden too.
            let room = match &decision {
                RunDecision::Event(room) => {
                    let mut room = (*room).clone();
                    room.rng = Rng::new(step ^ 0xE7E7);
                    Some(room)
                }
                _ => None,
            };
            let hidden_decision = room
                .as_ref()
                .map_or_else(|| decision.clone(), RunDecision::Event);
            let hidden_combat = run.combat().map(|c| perturbed(c, step));
            let hidden_view = hidden_combat.as_ref().map(|combat| CombatView {
                combat,
                state: combat.state(),
            });
            assert!(
                base == observe_run(&hidden, &hidden_decision, hidden_view),
                "seed {seed} step {step} ({decision:?}): the run observation changed with hidden state"
            );
            positions += 1;
            let legal = run.legal_actions();
            if legal.is_empty() {
                break;
            }
            let nth = usize::try_from(policy.next() % legal.len() as u64).expect("below len");
            let _ = run.apply(legal[nth].clone());
        }
    }
    assert!(positions > 1_000, "{positions} positions");
    // Ancients, events, map, combat, rewards, rest sites and shops were all observed.
    for phase in [0, 1, 2, 3, 4, 7, 9] {
        assert!(kinds[phase] > 0, "phase {phase} never seen: {kinds:?}");
    }
}

#[test]
fn blindfolding_hidden_state_does_not_change_the_observation() {
    let combat = Start::w1_all(0).combat;
    let base = observe(&combat);
    let mut changed = 0;
    for seed in 0..8 {
        let other = combat.blindfold_for_search(seed);
        assert_eq!(
            base,
            observe(&other),
            "the observation changed for world {seed}"
        );
        if other.state().player.draw[..] != combat.state().player.draw[..] {
            changed += 1;
        }
    }
    assert!(
        changed > 0,
        "the blindfold did not change the hidden draw order"
    );
}

/// A blindfolded world is a function of what the player sees and the seed only: two fights that
/// differ in their hidden draw and discard order give the same world.
#[test]
fn a_blindfolded_world_does_not_depend_on_the_real_pile_order() {
    for seed in 0..16 {
        let mut combat = Start::w1_all(seed).combat;
        // Play into the fight so the discard pile has cards.
        let mut legal = Vec::new();
        for _ in 0..12 {
            combat.legal_actions_into(&mut legal);
            let Some(action) = legal.last().cloned() else {
                break;
            };
            combat.apply(action).expect("a listed action applies");
        }
        let mut other = combat.clone();
        other.edit(|s| {
            s.player.draw.reverse();
            s.player.discard.reverse();
        });
        for world in 0..4 {
            let a = combat.blindfold_for_search(world);
            let b = other.blindfold_for_search(world);
            assert_eq!(
                a.state().player.draw[..],
                b.state().player.draw[..],
                "seed {seed}"
            );
            assert_eq!(
                a.state().player.discard[..],
                b.state().player.discard[..],
                "seed {seed}"
            );
        }
    }
}

/// The privileged buffer of the given parts with a fresh writer.
fn priv_of(s: &RunState, run_rng: &RunRngSet, combat: Option<&CombatState>) -> Vec<u8> {
    let mut out = vec![0; PRIV_BYTES];
    PrivObserver::default().write_parts(s, run_rng, combat, &mut out);
    out
}

/// H1, the positive control (`docs/plans/necro-e2e.md`, section 5): the perturbations of
/// [`hidden_run_state_does_not_reach_the_run_observation`] leave the observation's bytes unchanged
/// and change the privileged buffer's in at least 95% of the sampled states, so a leak into the
/// observation would be seen.
#[test]
fn hidden_state_changes_the_privileged_buffer_but_not_the_observation() {
    let mut positions = 0_u32;
    let mut changed = 0_u32;
    let mut fights = (0_u32, 0_u32);
    for seed in 0..24_u64 {
        let mut run = RunConfig::default().run(seed).expect("a run");
        let mut policy = Policy(seed << 1 | 1);
        for step in 0..400_u64 {
            let decision = run.pending_decision();
            if matches!(
                decision,
                RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_)
            ) {
                break;
            }
            let s = run.state();
            let view = run.combat().map(|combat| CombatView {
                combat,
                state: combat.state(),
            });
            let base = observe_run(s, &decision, view);
            let base_priv = priv_of(s, run.rng(), run.combat().map(Combat::state));
            let other = random_run(seed + 1_000 + step, step % 97);
            let hidden = perturbed_run(s, other.state());
            let hidden_combat = run.combat().map(|c| perturbed(c, step));
            let hidden_view = hidden_combat.as_ref().map(|combat| CombatView {
                combat,
                state: combat.state(),
            });
            assert!(
                base == observe_run(&hidden, &decision, hidden_view),
                "seed {seed} step {step} ({decision:?}): the run observation changed with hidden state"
            );
            let hidden_rng = hidden_combat
                .as_ref()
                .map_or(&hidden.rng, |c| &c.state().rng);
            let differs = base_priv
                != priv_of(
                    &hidden,
                    hidden_rng,
                    hidden_combat.as_ref().map(Combat::state),
                );
            positions += 1;
            changed += u32::from(differs);
            if run.combat().is_some() {
                fights.0 += 1;
                fights.1 += u32::from(differs);
            }
            let legal = run.legal_actions();
            if legal.is_empty() {
                break;
            }
            let nth = usize::try_from(policy.next() % legal.len() as u64).expect("below len");
            let _ = run.apply(legal[nth].clone());
        }
    }
    assert!(positions > 1_000, "{positions} positions");
    assert!(
        changed * 100 >= positions * 95,
        "the privileged buffer changed in {changed} of {positions} states"
    );
    assert!(fights.0 > 100, "{} fight states", fights.0);
    assert!(
        fights.1 * 100 >= fights.0 * 95,
        "changed in {} of {} fight states",
        fights.1,
        fights.0
    );
    eprintln!(
        "priv changed in {changed}/{positions} states ({}/{} in fights)",
        fights.1, fights.0
    );
}

/// The v2 section (RLN16: intent links, the forecast, the hand's previews) of `combat` with fresh
/// observers.
fn observe_v2(combat: &Combat) -> Vec<u8> {
    let mut obs = vec![0; OBS_BYTES];
    let mut observer = Observer::new(true);
    observer.observe(combat, &mut obs);
    let mut out = vec![0; V2_BYTES];
    V2Observer::default().write(combat, observer.intent_owners(), &mut out);
    out
}

/// The honest forecast (RLN16): reordering the draw pile and reseeding every RNG stream leaves
/// the v2 section's bytes unchanged at every position of random play, and the forecast shows
/// both certain and uncertain moves.
#[test]
fn hidden_state_does_not_reach_the_forecast() {
    let mut positions = 0;
    let (mut certain, mut uncertain, mut aimed) = (0, 0, 0);
    let mut legal = Vec::new();
    for (seed, rep) in (0..110).flat_map(|seed| [(seed, 0), (seed, 1)]) {
        let mut combat = Start::w1_all(seed).combat;
        let mut policy = Policy(seed << 1 | rep);
        for step in 0..120 {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            let base = observe_v2(&combat);
            for salt in [step, step ^ 0xABCD] {
                assert!(
                    base == observe_v2(&perturbed(&combat, salt)),
                    "seed {seed} rep {rep} step {step}: the v2 section changed with hidden state"
                );
            }
            for slot in 0..CREATURES * STEPS {
                let at = FORECAST_AT + slot * STEP_BYTES;
                if base[at] > 0 {
                    if base[at + 1] == u8::MAX {
                        certain += 1;
                    } else {
                        uncertain += 1;
                    }
                }
            }
            aimed += usize::from(base[AIMED_AT..SIDE_AT].iter().any(|&b| b != 0));
            positions += 1;
            combat.legal_actions_into(&mut legal);
            if legal.is_empty() {
                break;
            }
            let count = u64::try_from(legal.len()).expect("fits u64");
            let nth = usize::try_from(policy.next() % count).expect("below count");
            let action = legal.swap_remove(nth);
            combat.apply(action).expect("a listed action applies");
        }
    }
    assert!(positions > 4_000, "{positions} positions");
    assert!(
        certain > 1_000 && uncertain > 1_000,
        "{certain} certain and {uncertain} uncertain forecast moves"
    );
    assert!(aimed > positions / 2, "{aimed} positions with aimed damage");
}

/// The run observation with the v2 section on (RLN16) does not leak hidden run state either.
#[test]
fn hidden_run_state_does_not_reach_the_v2_run_observation() {
    let mut fights = 0;
    for seed in 0..8_u64 {
        let mut run = RunConfig::default().run(seed).expect("a run");
        let mut policy = Policy(seed << 1 | 1);
        for step in 0..400_u64 {
            let decision = run.pending_decision();
            if matches!(
                decision,
                RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_)
            ) {
                break;
            }
            let observe = |s: &RunState, combat: Option<&Combat>| {
                let mut out = vec![0; RUN_OBS_BYTES];
                let view = combat.map(|combat| CombatView {
                    combat,
                    state: combat.state(),
                });
                RunObserver::new(true)
                    .with_v2(true)
                    .observe_parts(s, &decision, view, &mut out);
                out
            };
            let base = observe(run.state(), run.combat());
            let other = random_run(seed + 1_000 + step, step % 97);
            let hidden = perturbed_run(run.state(), other.state());
            let hidden_combat = run.combat().map(|c| perturbed(c, step));
            assert!(
                base == observe(&hidden, hidden_combat.as_ref()),
                "seed {seed} step {step}: the v2 run observation changed with hidden state"
            );
            fights += usize::from(base[V2_AT..].iter().any(|&b| b != 0));
            let legal = run.legal_actions();
            if legal.is_empty() {
                break;
            }
            let nth = usize::try_from(policy.next() % legal.len() as u64).expect("below len");
            let _ = run.apply(legal[nth].clone());
        }
    }
    assert!(fights > 100, "{fights} fight positions with a v2 section");
}

/// RLN22: the plan column ([`plan_column`]) is the same for two positions that differ only in
/// hidden state (draw pile order, every RNG stream), at every position of random play on the RL
/// bench's starts, in both the player's turn and card choices. The control: the same planner on
/// the real state ([`plan_column_peeking`]) does change with it at some positions, so the test
/// can see a leak.
#[test]
fn hidden_state_does_not_reach_the_plan_column() {
    // Each position searches six turns; a debug build plays fewer seeds.
    let seeds: u64 = if cfg!(debug_assertions) { 30 } else { 110 };
    let (mut positions, mut choices, mut peeked) = (0, 0, 0);
    let mut legal = Vec::new();
    for seed in 0..seeds {
        let mut combat = Start::w1_all(seed).combat;
        let mut policy = Policy(seed << 3 | 5);
        let big = seed % 2 == 0;
        for step in 0..80 {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            combat.legal_actions_into(&mut legal);
            if legal.is_empty() {
                break;
            }
            // The full search on seeds 0, 1 mod 4 (so on big and normal fights), else the short.
            let depth = if seed % 4 < 2 {
                FULL_DEPTH
            } else {
                SHORT_DEPTH
            };
            let base = plan_column(&combat, big, &legal, depth);
            let peek = plan_column_peeking(&combat, big, &legal, depth);
            let mut seen = false;
            for salt in [step, step ^ 0xABCD] {
                let other = perturbed(&combat, salt);
                assert_eq!(
                    base,
                    plan_column(&other, big, &legal, depth),
                    "seed {seed} step {step}: the plan column changed with hidden state"
                );
                seen |= peek != plan_column_peeking(&other, big, &legal, depth);
            }
            peeked += usize::from(seen);
            choices += usize::from(combat.status() == Status::Decision);
            positions += 1;
            let count = u64::try_from(legal.len()).expect("fits u64");
            let nth = usize::try_from(policy.next() % count).expect("below count");
            let action = legal.swap_remove(nth);
            combat.apply(action).expect("a listed action applies");
        }
    }
    println!("{positions} positions, {choices} card choices, the control leaked at {peeked}");
    let per_seed = u64::try_from(positions).expect("fits") / seeds;
    assert!(per_seed >= 20, "{positions} positions");
    assert!(choices * 200 > positions, "{choices} card choices");
    // The score reads HP, block and debuffs, which the hidden state reaches only through draws
    // and random effects inside the turn: the control leaks at ~3% of positions (76 of 2,607).
    assert!(
        peeked * 100 > positions,
        "the control saw a leak at only {peeked} of {positions}"
    );
}

/// RLN22: the observation's plan section ([`write_plan`], as the run env writes it from the
/// mask) is the same with the fight's hidden state changed, at every combat decision of random
/// runs.
#[test]
fn hidden_state_does_not_reach_the_plan_section() {
    let (mut positions, mut scored) = (0, 0);
    let mut mask = RunMask::new();
    for seed in 0..16_u64 {
        let mut run = RunConfig::default().run(seed).expect("a run");
        let mut policy = Policy(seed << 2 | 3);
        for step in 0..600_u64 {
            let legal = run.legal_actions();
            if legal.is_empty() {
                break;
            }
            if let RunDecision::Combat(combat) = run.pending_decision() {
                mask.fill_run(&run, &legal);
                let actions: Vec<_> = (0..ACTIONS)
                    .filter_map(|i| match &legal[mask.position(i)?] {
                        sts2_core::run::RunAction::Combat(a) => Some((i, a.clone())),
                        _ => None,
                    })
                    .collect();
                let big = big_fight(&run);
                let mut base = vec![0; ACTIONS * PLAN_BYTES];
                write_plan(combat, big, &actions, &mut base);
                let mut other = vec![0; ACTIONS * PLAN_BYTES];
                write_plan(&perturbed(combat, step), big, &actions, &mut other);
                assert!(
                    base == other,
                    "seed {seed} step {step}: the plan section changed with hidden state"
                );
                positions += 1;
                scored += base
                    .chunks(PLAN_BYTES)
                    .filter(|e| e[2] & PLAN_SCORED != 0)
                    .count();
            }
            let nth = usize::try_from(policy.next() % legal.len() as u64).expect("below len");
            let _ = run.apply(legal[nth].clone());
        }
    }
    println!("{positions} combat decisions, {scored} scored actions");
    assert!(positions > 300, "{positions} combat decisions");
    assert!(scored > positions, "{scored} scored actions");
}

/// RLN32: the same at `plan_depth` 2 ([`write_plan_at`] with [`SHORT_DEPTH`]): the observation's
/// plan section is the same with the fight's hidden state changed, at every combat decision of random
/// runs.
#[test]
fn hidden_state_does_not_reach_the_depth_two_plan_section() {
    let (mut positions, mut scored) = (0, 0);
    let mut mask = RunMask::new();
    for seed in 0..8_u64 {
        let mut run = RunConfig::default().run(seed).expect("a run");
        let mut policy = Policy(seed << 2 | 3);
        for step in 0..600_u64 {
            let legal = run.legal_actions();
            if legal.is_empty() {
                break;
            }
            if let RunDecision::Combat(combat) = run.pending_decision() {
                mask.fill_run(&run, &legal);
                let actions: Vec<_> = (0..ACTIONS)
                    .filter_map(|i| match &legal[mask.position(i)?] {
                        sts2_core::run::RunAction::Combat(a) => Some((i, a.clone())),
                        _ => None,
                    })
                    .collect();
                let big = big_fight(&run);
                let mut base = vec![0; ACTIONS * PLAN_BYTES];
                write_plan_at(combat, big, &actions, SHORT_DEPTH, &mut base);
                let mut other = vec![0; ACTIONS * PLAN_BYTES];
                write_plan_at(
                    &perturbed(combat, step),
                    big,
                    &actions,
                    SHORT_DEPTH,
                    &mut other,
                );
                assert!(
                    base == other,
                    "seed {seed} step {step}: the plan section changed with hidden state"
                );
                positions += 1;
                scored += base
                    .chunks(PLAN_BYTES)
                    .filter(|e| e[2] & PLAN_SCORED != 0)
                    .count();
            }
            let nth = usize::try_from(policy.next() % legal.len() as u64).expect("below len");
            let _ = run.apply(legal[nth].clone());
        }
    }
    println!("{positions} combat decisions, {scored} scored actions");
    assert!(positions > 150, "{positions} combat decisions");
    assert!(scored > positions, "{scored} scored actions");
}
