//! The action-leak gate (`docs/specs/honesty.md`, test 2): at every position of the honest bot's
//! runs, the bot is asked again with what the player cannot see replaced: the run's and the
//! player's RNG streams, the reward odds, both relic bags, each act's event and encounter lists,
//! an event's own RNG, and in a fight the draw order and every RNG stream (the run's and each
//! monster's). The action must not change. A deliberately clairvoyant policy (one that reads the
//! draw pile and the run's RNG) must be caught by the same check. The chance-node tree (RLB7),
//! its learned playout policy and its priors have their own cases, as has the `AlphaZero` tree
//! (RLB13): its world seeds, folds and replays must see only the visible position.

use sts2_core::Character;
use sts2_core::Combat;
use sts2_core::Status;
use sts2_core::combat::CombatState;
use sts2_core::content::cards::CardType;
use sts2_core::rng::{Rng, RunRngSet};
use sts2_core::run::{Run, RunAction, RunDecision, RunState};
use sts2_diff::honest::az::{self, AzConfig};
use sts2_diff::honest::rollout::{RolloutNet, Scratch};
use sts2_diff::honest::search::{self, Playout, SearchConfig};
use sts2_diff::honest::tree::{self, TreeConfig};
use sts2_diff::honest::value::ValueNet;
use sts2_diff::honest::{Bot, Controller, new_run, ppo};

/// A policy as the gate calls it.
type Policy<'a> = &'a dyn Fn(&RunState, &RunDecision<'_>, &[RunAction]) -> RunAction;

/// `combat` with the draw pile reversed and rotated and every RNG stream reseeded from `salt`.
fn perturbed_combat(combat: &Combat, salt: u64) -> Combat {
    let mut combat = combat.clone();
    combat.edit(|s: &mut CombatState| {
        let draw = &mut s.player.draw;
        draw.reverse();
        if draw.len() > 2 {
            let by = usize::try_from(salt).expect("a 64-bit usize") % draw.len();
            draw.rotate_left(by);
        }
        s.rng = RunRngSet::new(&format!("ACTIONLEAK{salt}"));
        for (i, creature) in s.creatures.iter_mut().enumerate() {
            if let Some(monster) = &mut creature.monster {
                monster.rng = Rng::new(salt ^ (0xA11 << 20) ^ u64::try_from(i).expect("fits"));
            }
        }
    });
    combat
}

/// `s` with its hidden state taken from `other` (as `sts2-rl`'s observation leak test).
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

/// A fast honest search for driving the runs and for the gate.
const fn search_bot() -> Bot {
    Bot {
        controller: Controller::Search(SearchConfig {
            worlds: 3,
            big_worlds: 3,
            boss_worlds: 3,
            playouts: true,
            net_playouts: false,
            beam: 4,
            lookahead: 0,
            beam2: 2,
            depth: 1,
            lookahead_in: search::Scope::Big,
            close: 0,
            close_worlds: 1,
            value: search::ValueUse::DEFAULT,
        }),
        net: None,
        run_policy: None,
    }
}

/// Plays `seeds` with `driver` for up to `steps` actions each; at every position asks `policy`
/// on the real state and on the state with its hidden part replaced. Returns the positions
/// checked and the first position where the action changed.
fn first_leak(
    policy: Policy<'_>,
    driver: &Bot,
    seeds: &[u64],
    steps: usize,
) -> (usize, Option<String>) {
    let others: Vec<Run> = (0..4)
        .map(|i| {
            let mut run = new_run(Character::Ironclad, 7_000_000 + i, 0).expect("a run");
            for _ in 0..(40 * i) {
                let legal = run.legal_actions();
                if legal.is_empty() {
                    break;
                }
                let a = driver.choose(run.state(), &run.pending_decision(), &legal);
                let _ = run
                    .apply(a.clone())
                    .or_else(|_| run.apply(legal[0].clone()));
            }
            run
        })
        .collect();
    let mut positions = 0;
    for &seed in seeds {
        let mut run = new_run(Character::Ironclad, seed, 0).expect("a run");
        for step in 0..steps {
            let decision = run.pending_decision();
            if matches!(
                decision,
                RunDecision::GameOver | RunDecision::Victory | RunDecision::Stopped(_)
            ) {
                break;
            }
            let legal = run.legal_actions();
            if legal.is_empty() {
                break;
            }
            let real = policy(run.state(), &decision, &legal);
            let salt = seed ^ (step as u64) << 8;
            let hidden = perturbed_run(run.state(), others[step % others.len()].state());
            let room = match &decision {
                RunDecision::Event(room) => {
                    let mut room = (*room).clone();
                    room.rng = Rng::new(salt ^ 0xE7E7);
                    Some(room)
                }
                _ => None,
            };
            let combat = run.combat().map(|c| perturbed_combat(c, salt));
            let hidden_decision = match (&room, &combat) {
                (Some(room), _) => RunDecision::Event(room),
                (None, Some(combat)) if matches!(decision, RunDecision::Combat(_)) => {
                    RunDecision::Combat(combat)
                }
                _ => decision.clone(),
            };
            let blind = policy(&hidden, &hidden_decision, &legal);
            positions += 1;
            if blind != real {
                return (
                    positions,
                    Some(format!(
                        "seed {seed} step {step} (phase {}): {real:?} became {blind:?}",
                        sts2_rl::run_obs::phase_code(&decision)
                    )),
                );
            }
            let action = driver.choose(run.state(), &decision, &legal);
            drop(decision);
            if run.apply(action).is_err() {
                let Some(a) = legal.iter().find(|a| run.accepts(a)) else {
                    break;
                };
                run.apply(a.clone()).expect("an accepted action");
            }
        }
    }
    (positions, None)
}

const SEEDS: [u64; 2] = [8_000_003, 8_000_017];
const STEPS: usize = 220;

#[test]
fn the_honest_search_bot_does_not_read_hidden_state() {
    let bot = search_bot();
    let (positions, leak) = first_leak(&|s, d, l| bot.choose(s, d, l), &bot, &SEEDS, STEPS);
    assert_eq!(leak, None, "after {positions} positions");
    assert!(positions > 300, "{positions} positions");
}

/// The PPO controller, when its exported weights are present (`target/` is not committed).
#[test]
fn the_honest_ppo_bot_does_not_read_hidden_state() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/combat-ppo.bin");
    let Ok(net) = ppo::Net::load(path) else {
        return;
    };
    let bot = Bot {
        controller: Controller::Ppo(std::sync::Arc::new(net)),
        net: None,
        run_policy: None,
    };
    let (positions, leak) =
        first_leak(&|s, d, l| bot.choose(s, d, l), &search_bot(), &SEEDS, STEPS);
    assert_eq!(leak, None, "after {positions} positions");
}

/// The gate catches a fight policy that peeks: it plays differently when the top of the draw
/// pile is an attack (and is the honest bot elsewhere).
#[test]
fn a_policy_that_reads_the_draw_order_is_caught() {
    let bot = search_bot();
    let peek = |s: &RunState, d: &RunDecision<'_>, legal: &[RunAction]| {
        let RunDecision::Combat(combat) = d else {
            return bot.choose(s, d, legal);
        };
        let c = combat.state();
        let attack_on_top = c
            .player
            .draw
            .first()
            .is_some_and(|&id| c.card(id).kind.card_type() == CardType::Attack);
        if attack_on_top {
            legal[legal.len() - 1].clone()
        } else {
            legal[0].clone()
        }
    };
    let (_, leak) = first_leak(&peek, &bot, &SEEDS, STEPS);
    assert!(
        leak.is_some(),
        "the gate missed a policy that reads the draw order"
    );
}

/// The gate catches a run policy that peeks: out of a fight it picks by the run's hidden RNG
/// (and is the honest bot in fights).
#[test]
fn a_policy_that_reads_the_run_rng_is_caught() {
    let bot = search_bot();
    let peek = |s: &RunState, d: &RunDecision<'_>, legal: &[RunAction]| {
        if matches!(d, RunDecision::Combat(_)) {
            return bot.choose(s, d, legal);
        }
        let h = format!("{:?}", s.rng).bytes().fold(0_usize, |h, b| {
            h.wrapping_mul(31).wrapping_add(usize::from(b))
        });
        legal[h % legal.len()].clone()
    };
    let (_, leak) = first_leak(&peek, &bot, &SEEDS, STEPS);
    assert!(
        leak.is_some(),
        "the gate missed a policy that reads the run's RNG"
    );
}

/// The search with its full-fight playouts (as in an elite or boss fight) at every position of
/// RL bench fights: the hidden draw order and RNG streams do not change its action.
#[test]
fn the_playout_search_does_not_read_hidden_state() {
    let config = SearchConfig {
        worlds: 2,
        big_worlds: 2,
        boss_worlds: 2,
        playouts: true,
        net_playouts: false,
        beam: 3,
        lookahead: 0,
        beam2: 2,
        depth: 1,
        lookahead_in: search::Scope::Big,
        close: 0,
        close_worlds: 1,
        value: search::ValueUse::DEFAULT,
    };
    assert!(search_reads_no_hidden_state(config) > 100);
}

/// The multi-turn lookahead (RLB10: two ends per first action, planned two more turns, and in
/// common fights too) at every position of RL bench fights.
#[test]
fn the_lookahead_search_does_not_read_hidden_state() {
    let config = SearchConfig {
        worlds: 2,
        big_worlds: 2,
        boss_worlds: 2,
        playouts: true,
        net_playouts: false,
        beam: 3,
        lookahead: 2,
        beam2: 2,
        depth: 2,
        lookahead_in: search::Scope::All,
        close: 0,
        close_worlds: 1,
        value: search::ValueUse::DEFAULT,
    };
    assert!(search_reads_no_hidden_state(config) > 100);
}

/// The lookahead bot over whole runs, commons included, the lookahead on close decisions.
#[test]
fn the_lookahead_bot_does_not_read_hidden_state() {
    let bot = Bot {
        controller: Controller::Search(SearchConfig {
            worlds: 2,
            big_worlds: 2,
            boss_worlds: 2,
            playouts: true,
            net_playouts: false,
            beam: 3,
            lookahead: 2,
            beam2: 2,
            depth: 1,
            lookahead_in: search::Scope::All,
            close: 300,
            close_worlds: 1,
            value: search::ValueUse::DEFAULT,
        }),
        net: None,
        run_policy: None,
    };
    let (positions, leak) = first_leak(&|s, d, l| bot.choose(s, d, l), &bot, &SEEDS, STEPS);
    assert_eq!(leak, None, "after {positions} positions");
    assert!(positions > 300, "{positions} positions");
}

/// The value net's search (RLB11) at every position of RL bench fights: its leaves scored by
/// the net with a two-turn lookahead, and its playouts by the net's policy.
#[test]
fn the_value_search_does_not_read_hidden_state() {
    let net = ValueNet::builtin().expect("the committed value net loads");
    let mut config = SearchConfig {
        worlds: 2,
        big_worlds: 2,
        boss_worlds: 2,
        playouts: true,
        net_playouts: false,
        beam: 3,
        lookahead: 1,
        beam2: 2,
        depth: 1,
        lookahead_in: search::Scope::All,
        close: 0,
        close_worlds: 1,
        value: search::ValueUse {
            leaves: true,
            keep: 2,
            ..search::ValueUse::DEFAULT
        },
    };
    assert!(policy_reads_no_hidden_state(config, Playout::Value(&net)) > 100);
    config.value.leaves = false;
    config.lookahead = 0;
    assert!(policy_reads_no_hidden_state(config, Playout::Value(&net)) > 100);
}

/// The value net's search over whole runs, commons included.
#[test]
fn the_value_bot_does_not_read_hidden_state() {
    let net = std::sync::Arc::new(ValueNet::builtin().expect("the committed value net loads"));
    let bot = Bot {
        controller: Controller::SearchValue(
            SearchConfig {
                worlds: 2,
                big_worlds: 2,
                boss_worlds: 2,
                beam: 3,
                value: search::ValueUse {
                    leaves: true,
                    keep: 2,
                    ..search::ValueUse::DEFAULT
                },
                ..SearchConfig::default()
            },
            net,
            None,
        ),
        net: None,
        run_policy: None,
    };
    let (positions, leak) = first_leak(&|s, d, l| bot.choose(s, d, l), &bot, &SEEDS, STEPS);
    assert_eq!(leak, None, "after {positions} positions");
    assert!(positions > 300, "{positions} positions");
}

/// The web client's bot (WEB10, `Bot::advise`): the champion's search (`advice::PHONE`'s, at
/// fewer worlds and a narrower beam for the test's time) and the learned run policy, over whole
/// runs. Neither its move nor its reason (a fight's win chance, a choice's confidence) changes
/// when only hidden state does.
#[test]
fn the_web_bot_and_its_reasons_do_not_read_hidden_state() {
    use sts2_diff::honest::advice::{PHONE, Why, champion_search};
    let mut search = champion_search(2, 2, 2);
    search.beam = 6;
    search.value.keep = 2;
    assert_eq!(search.value.mix, PHONE.value.mix);
    let bot = Bot::champion(search);
    let whys = std::cell::RefCell::new(Vec::<Why>::new());
    let advise = |s: &RunState, d: &RunDecision<'_>, l: &[RunAction]| {
        let (action, why) = bot.advise(s, d, l);
        whys.borrow_mut().push(why);
        action
    };
    let (positions, leak) = first_leak(&advise, &search_bot(), &SEEDS[..1], STEPS);
    assert_eq!(leak, None, "after {positions} positions");
    assert!(positions > 150, "{positions} positions");
    // `first_leak` asks on the real state, then on the hidden one: the reasons come in pairs.
    let whys = whys.into_inner();
    let pair = whys.chunks(2).position(|p| p[0] != p[1]);
    assert_eq!(pair, None, "the reason changed: {whys:?}");
    assert!(whys.iter().any(|w| matches!(w, Why::Win(_))));
    assert!(whys.iter().any(|w| matches!(w, Why::Sure(_))));
}

/// stopped after a turn.
#[test]
fn the_mixed_search_does_not_read_hidden_state() {
    let net = ValueNet::builtin().expect("the committed value net loads");
    let rollout = RolloutNet::builtin();
    let mut config = SearchConfig {
        worlds: 2,
        big_worlds: 2,
        boss_worlds: 2,
        beam: 3,
        value: search::ValueUse {
            leaves: true,
            keep: 2,
            mix: 500,
            mix_ends: 2,
            ..search::ValueUse::DEFAULT
        },
        ..SearchConfig::default()
    };
    let policy = Playout::Mixed(&net, &rollout);
    assert!(policy_reads_no_hidden_state(config, policy) > 100);
    config.value.mix_turns = 1;
    assert!(policy_reads_no_hidden_state(config, policy) > 100);
}

/// RLB15: several playouts of each end, sampled, by the v3 net (it reads which fight it is).
#[test]
fn the_sampled_mixed_search_does_not_read_hidden_state() {
    let net = ValueNet::builtin().expect("the committed value net loads");
    let rollout = RolloutNet::v2().with_encounters();
    let config = SearchConfig {
        worlds: 2,
        big_worlds: 2,
        boss_worlds: 2,
        beam: 3,
        value: search::ValueUse {
            leaves: true,
            keep: 2,
            mix: 500,
            mix_samples: 3,
            ..search::ValueUse::DEFAULT
        },
        ..SearchConfig::default()
    };
    assert!(policy_reads_no_hidden_state(config, Playout::Mixed(&net, &rollout)) > 100);
}

/// Plays RL bench fights with `config` as in a boss fight, checking at every position that the
/// hidden draw order and RNG streams do not change its action; the positions checked.
fn search_reads_no_hidden_state(config: SearchConfig) -> usize {
    policy_reads_no_hidden_state(config, Playout::Priority)
}

/// [`search_reads_no_hidden_state`] with the playouts (or leaves) by `policy`.
fn policy_reads_no_hidden_state(config: SearchConfig, policy: Playout<'_>) -> usize {
    let context = search::FightContext {
        big: true,
        boss: true,
        act: 0,
    };
    let mut positions = 0;
    for seed in 0..10 {
        let mut combat = sts2_rl::Start::w1_all(seed).combat;
        for step in 0..30_u64 {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            let legal = combat.legal_actions();
            let real = search::choose_with(&combat, &legal, context, config, policy);
            let blind = search::choose_with(
                &perturbed_combat(&combat, seed << 8 | step),
                &legal,
                context,
                config,
                policy,
            );
            assert_eq!(real, blind, "seed {seed} step {step}");
            positions += 1;
            combat.apply(real).expect("a legal action");
        }
    }
    positions
}

/// The chance-node tree (RLB7) with its learned playouts and priors, small budgets.
fn tree_bot() -> Bot {
    Bot {
        controller: Controller::Tree(
            TreeConfig {
                sims: 24,
                big_sims: 24,
                boss_sims: 24,
                ..TreeConfig::default()
            },
            Some(std::sync::Arc::new(RolloutNet::builtin())),
        ),
        net: None,
        run_policy: None,
    }
}

#[test]
fn the_tree_bot_does_not_read_hidden_state() {
    let bot = tree_bot();
    let (positions, leak) = first_leak(&|s, d, l| bot.choose(s, d, l), &bot, &SEEDS, STEPS);
    assert_eq!(leak, None, "after {positions} positions");
    assert!(positions > 300, "{positions} positions");
}

/// The tree alone (no hybrid: common fights too), with playouts and priors, at every position
/// of RL bench fights.
#[test]
fn the_tree_search_does_not_read_hidden_state() {
    let net = RolloutNet::builtin();
    let config = TreeConfig {
        sims: 24,
        big_sims: 24,
        boss_sims: 24,
        hybrid: false,
        playouts_all: true,
        ..TreeConfig::default()
    };
    let mut positions = 0;
    for (seed, big) in (0..10).zip([true, false].into_iter().cycle()) {
        let context = search::FightContext {
            big,
            boss: big,
            act: 0,
        };
        let mut combat = sts2_rl::Start::w1_all(seed).combat;
        for step in 0..30_u64 {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            let legal = combat.legal_actions();
            let real = tree::choose(&combat, &legal, context, config, Playout::Learned(&net));
            let blind = tree::choose(
                &perturbed_combat(&combat, seed << 8 | step),
                &legal,
                context,
                config,
                Playout::Learned(&net),
            );
            assert_eq!(real, blind, "seed {seed} step {step}");
            positions += 1;
            combat.apply(real).expect("a legal action");
        }
    }
    assert!(positions > 100, "{positions} positions");
}

/// The `AlphaZero` tree (RLB13) at every position of RL bench fights, with a budget that folds
/// outcomes (so replays run): its action and its root visits must not change.
#[test]
fn the_az_search_does_not_read_hidden_state() {
    let net = ValueNet::builtin().expect("the committed value net loads");
    let config = AzConfig {
        sims: 48,
        big_sims: 48,
        boss_sims: 48,
        turns: 3,
        widen_k: 0.5,
        end_k: 0.5,
        ..AzConfig::default()
    };
    let mut positions = 0;
    for (seed, big) in (0..10).zip([true, false].into_iter().cycle()) {
        let context = search::FightContext {
            big,
            boss: big,
            act: 0,
        };
        let mut combat = sts2_rl::Start::w1_all(seed).combat;
        for step in 0..30_u64 {
            if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                break;
            }
            let legal = combat.legal_actions();
            let visits = |c: &Combat| -> Vec<u32> {
                az::search_root(c, &legal, context, config, &net)
                    .into_iter()
                    .map(|(_, v, _)| v)
                    .collect()
            };
            let blind = perturbed_combat(&combat, seed << 8 | step);
            assert_eq!(visits(&combat), visits(&blind), "seed {seed} step {step}");
            let real = az::choose(&combat, &legal, context, config, &net);
            assert_eq!(real, az::choose(&blind, &legal, context, config, &net));
            positions += 1;
            combat.apply(real).expect("a legal action");
        }
    }
    assert!(positions > 100, "{positions} positions");
}

/// The `AlphaZero` bot over whole runs, commons included.
#[test]
fn the_az_bot_does_not_read_hidden_state() {
    let net = std::sync::Arc::new(ValueNet::builtin().expect("the committed value net loads"));
    let bot = Bot {
        controller: Controller::Az(
            AzConfig {
                sims: 64,
                big_sims: 64,
                boss_sims: 64,
                scope: search::Scope::All,
                ..AzConfig::default()
            },
            SearchConfig::default(),
            net,
        ),
        net: None,
        run_policy: None,
    };
    let (positions, leak) = first_leak(&|s, d, l| bot.choose(s, d, l), &bot, &SEEDS, STEPS);
    assert_eq!(leak, None, "after {positions} positions");
    // A 64-simulation tree in every fight loses one run early.
    assert!(positions > 200, "{positions} positions");
}

/// The learned playout policy's move (greedy) and its priors do not change with the hidden
/// draw order and RNG streams.
#[test]
fn the_learned_playout_policy_does_not_read_hidden_state() {
    let net = RolloutNet::builtin();
    let mut scratch = Scratch::default();
    let mut positions = 0;
    for seed in 0..20 {
        let mut combat = sts2_rl::Start::w1_all(seed).combat;
        for step in 0..40_u64 {
            if combat.status() != Status::PlayerTurn {
                break;
            }
            let legal = combat.legal_actions();
            let blind = perturbed_combat(&combat, seed << 8 | step);
            let real = net.choose(&combat, &legal, &mut scratch, None);
            assert_eq!(real, net.choose(&blind, &legal, &mut scratch, None));
            let (mut p, mut q) = (Vec::new(), Vec::new());
            net.priors(&combat, &legal, 1.0, &mut scratch, &mut p);
            net.priors(&blind, &legal, 1.0, &mut scratch, &mut q);
            assert_eq!(p, q, "seed {seed} step {step}");
            positions += 1;
            let Some(action) = real.or_else(|| legal.first().cloned()) else {
                break;
            };
            combat.apply(action).expect("a legal action");
        }
    }
    assert!(positions > 100, "{positions} positions");
}

/// Plays `seeds` with `driver` and asks the run policy `bot` at every run decision (not in a
/// fight) on the real state and on the state with its hidden part replaced; the run decisions
/// checked and the first leak.
fn first_run_policy_leak(bot: &Bot, driver: &Bot, seeds: &[u64]) -> (usize, Option<String>) {
    let checked = std::cell::Cell::new(0);
    let policy = |s: &RunState, d: &RunDecision<'_>, legal: &[RunAction]| {
        if matches!(d, RunDecision::Combat(_)) {
            return legal[0].clone();
        }
        checked.set(checked.get() + 1);
        bot.choose(s, d, legal)
    };
    let (_, leak) = first_leak(&policy, driver, seeds, 2_500);
    (checked.get(), leak)
}

/// The learned run policy (RLB8): the committed weights and a random net, each making the run
/// decisions of its own runs (the fights by a one-world search, for speed), never change a run
/// decision when only hidden state changes.
#[test]
fn the_run_policy_does_not_read_hidden_state() {
    use sts2_diff::honest::policy::RunNet;
    let fights = Controller::Search(SearchConfig {
        worlds: 1,
        big_worlds: 1,
        boss_worlds: 1,
        playouts: false,
        net_playouts: false,
        beam: 2,
        lookahead: 0,
        beam2: 2,
        depth: 1,
        lookahead_in: search::Scope::Big,
        close: 0,
        close_worlds: 1,
        value: search::ValueUse::DEFAULT,
    });
    for (net, seeds) in [
        (RunNet::builtin(), &[8_000_003, 8_000_017][..]),
        (RunNet::random(7), &[8_000_029][..]),
    ] {
        let bot = Bot {
            controller: fights.clone(),
            net: None,
            run_policy: Some(std::sync::Arc::new(net)),
        };
        let (checked, leak) = first_run_policy_leak(&bot, &bot, seeds);
        assert_eq!(leak, None, "after {checked} run decisions");
        assert!(checked > 40 * seeds.len(), "{checked} run decisions");
    }
}

/// RLB18: the deck probe's re-ranking of close run decisions (a wide margin, so most decisions
/// with two plain candidates are probed) never changes a run decision when only hidden state
/// changes: the boss is the map's, the worlds come from fixed seeds.
#[test]
fn the_deck_probe_does_not_read_hidden_state() {
    use sts2_diff::honest::{policy::RunNet, probe};
    let net = RunNet::builtin();
    let rollout = RolloutNet::v2();
    let config = probe::ProbeUse {
        worlds: 4,
        margin: 100.0,
        weight: 20.0,
    };
    let checked = std::cell::Cell::new(0);
    let policy = |s: &RunState, d: &RunDecision<'_>, legal: &[RunAction]| {
        if matches!(d, RunDecision::Combat(_)) {
            return legal[0].clone();
        }
        checked.set(checked.get() + 1);
        probe::choose_with(config, &rollout, &net, s, d, legal).unwrap_or_else(|| legal[0].clone())
    };
    let driver = Bot {
        controller: Controller::Search(SearchConfig {
            worlds: 1,
            big_worlds: 1,
            boss_worlds: 1,
            playouts: false,
            beam: 2,
            ..SearchConfig::default()
        }),
        net: None,
        run_policy: Some(std::sync::Arc::new(RunNet::builtin())),
    };
    let (_, leak) = first_leak(&policy, &driver, &[8_000_003], 2_500);
    assert_eq!(leak, None, "after {} run decisions", checked.get());
    assert!(checked.get() > 40, "{} run decisions", checked.get());
    assert!(probe::PROBED.load(std::sync::atomic::Ordering::Relaxed) > 5);
}

#[test]
fn the_mid_boss_settings_hold_in_act_1_and_2_boss_fights() {
    let mut config = SearchConfig::default();
    config.value.mid_boss_hp = Some(0);
    config.value.boss_potion = Some(0);
    let fight = |big, boss, act| search::FightContext { big, boss, act };
    let value = |context| config.in_fight(context).value;
    assert_eq!(value(fight(true, true, 0)).hp, 0);
    assert_eq!(value(fight(true, true, 1)).hp, 0);
    assert_eq!(value(fight(true, true, 2)).hp, config.value.hp);
    assert_eq!(value(fight(true, false, 1)).hp, config.value.hp);
    assert_eq!(value(fight(true, true, 2)).potion, 0);
    assert_eq!(value(fight(true, false, 2)).potion, config.value.potion);
}

#[test]
fn the_road_potion_cost_holds_outside_boss_fights_from_its_act() {
    let mut config = SearchConfig::default();
    config.value.road_potion = Some(25_000);
    config.value.road_act = 2;
    let fight = |big, boss, act| search::FightContext { big, boss, act };
    let cost = |context| config.in_fight(context).value.potion;
    let base = SearchConfig::default().value.potion;
    assert_eq!(cost(fight(true, false, 2)), 25_000);
    assert_eq!(cost(fight(false, false, 2)), 25_000);
    assert_eq!(cost(fight(true, true, 2)), base);
    assert_eq!(cost(fight(true, false, 1)), base);
    assert_eq!(
        SearchConfig::default().in_fight(fight(true, false, 2)),
        SearchConfig::default()
    );
}
