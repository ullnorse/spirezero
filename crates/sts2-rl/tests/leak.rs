//! The observation does not leak hidden state (`docs/reviews/spire-mentor.md`, idea 2): at
//! every position of random play on the RL bench's starts, reordering the draw pile and
//! reseeding every RNG stream (the run's streams and each monster's own) leaves the
//! observation's bytes unchanged. The same holds for the run env's observation at every
//! position of random runs, with the run's hidden state (its streams, odds, relic bags, the acts'
//! event and encounter lists, an event's own RNG) taken from another run.

use sts2_core::combat::CombatState;
use sts2_core::rng::{Rng, RunRngSet};
use sts2_core::run::{Run, RunDecision, RunState};
use sts2_core::{Combat, Status};
use sts2_rl::obs::{OBS_BYTES, Observer};
use sts2_rl::run_obs::{CombatView, RUN_OBS_BYTES, RunObserver};
use sts2_rl::{RunConfig, Start};

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
    RunObserver::new(true).observe_parts(s, decision, combat, &mut out);
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
