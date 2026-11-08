//! The run review's check (WEB46): plays finished runs as an imperfect player (the bot at one
//! world a decision, with a share of random moves), keeps a win, an early defeat and a late
//! defeat as saves in `$OUT` (the page loads them), and prints what the review finds in each.
//! Usage: `cargo run --release -p sts2-web-bot --example review_runs` (OUT, default target/web46/runs).

#![allow(
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::type_complexity,
    clippy::too_many_lines,
    reason = "a command-line check: it prints, and its table of wanted runs is read once"
)]

use serde_json::{Value, json};
use sts2_core::Action;
use sts2_core::run::RunAction;
use sts2_diff::honest::Bot;
use sts2_diff::honest::advice::champion_search;
use sts2_diff::honest::search::{FightContext, Searched};
use sts2_web::Game;
use sts2_web_bot::choose;
use sts2_web_bot::review::Review;

/// A finished run by the imperfect player: one move in `noise` is random (0: none).
fn play(player: &Bot, character: usize, seed: &str, noise: u64) -> Game {
    let mut game = Game::new(character, seed, 0, &[]).expect("run");
    let mut rng = seed
        .bytes()
        .fold(0x9E37_79B9_u64, |h, b| h.wrapping_mul(31) + u64::from(b))
        | 1;
    while !game.is_over() && game.actions().len() < 20_000 {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        let run = game.run();
        let mut legal = run.legal_actions();
        legal.retain(|a| run.accepts(a));
        let random = noise > 0 && legal.len() > 1 && rng % noise == 0;
        let action = if random {
            legal[usize::try_from((rng >> 8) % legal.len() as u64).expect("small")].clone()
        } else if let Some((action, _)) = choose(player, &game) {
            action
        } else {
            break;
        };
        game.act(action);
    }
    game
}

fn review(c: usize, seed: &str, actions: &[u32]) -> (Value, Value) {
    let list = serde_json::to_string(actions).expect("json");
    let mut r = Review::new(c, seed, 0, "[]", &list, 0, "[]", 0).expect("review");
    let t = std::time::Instant::now();
    while r.step().expect("step") {}
    eprintln!(
        "  reviewed {} actions in {:.0}s",
        r.total(),
        t.elapsed().as_secs_f64()
    );
    let state: Value = serde_json::from_str(&r.state()).expect("json");
    let result: Value = serde_json::from_str(&r.result().expect("result")).expect("json");
    (state, result)
}

/// Each pick against the bot's own answers, not the review's: the action the player took at
/// `at`, the move the bot makes there, the policy's confidence and the search's totals.
fn check_picks(c: usize, seed: &str, actions: &[u32], result: &Value) {
    let bot = sts2_web_bot::bot();
    for pick in result["picks"].as_array().expect("picks") {
        let at = usize::try_from(pick["at"].as_u64().expect("at")).expect("small");
        let game = Game::replay(c, seed, 0, &[], &actions[..at]).expect("replay");
        let list = serde_json::to_string(actions).expect("json");
        let board: Value = serde_json::from_str(
            &Review::new(c, seed, 0, "[]", &list, 0, "[]", 0)
                .expect("review")
                .board(at)
                .expect("board"),
        )
        .expect("json");
        assert_eq!(
            board["you"]["label"], pick["you"],
            "the move played at {at}"
        );
        assert_eq!(board["bot"]["label"], pick["bot"], "the bot's move at {at}");
        let (action, why) = choose(bot, &game).expect("a move");
        let run = game.run();
        let mut legal = run.legal_actions();
        legal.retain(|a| run.accepts(a));
        let decision = run.pending_decision();
        let mine = game.recordable_actions()[actions[at] as usize].clone();
        if let sts2_core::run::RunDecision::Combat(combat) = &decision {
            let RunAction::Combat(mine) = mine else {
                panic!("a fight's move")
            };
            let fight: Vec<Action> = legal
                .iter()
                .filter_map(|a| {
                    if let RunAction::Combat(a) = a {
                        Some(a.clone())
                    } else {
                        None
                    }
                })
                .collect();
            let room = pick["room"].as_str().expect("room");
            let context = FightContext {
                big: room != "fight",
                boss: room == "boss",
                act: run.state().act_index,
            };
            let Searched::Scored(scored) = bot.fight_scored(combat, &fight, context) else {
                panic!("no scores at {at}")
            };
            let total = |a: &Action| {
                scored.totals[scored.first.iter().position(|x| x == a).expect("searched")]
            };
            let worlds = i64::try_from(scored.worlds).expect("small");
            let RunAction::Combat(best) = &action else {
                panic!("a fight's move")
            };
            assert_eq!(scored.best(), best);
            let mean = |t: i64| t / worlds / 1000;
            assert_eq!(
                pick["score"]["you"].as_i64(),
                Some(mean(total(&mine))),
                "{pick}"
            );
            assert_eq!(
                pick["score"]["bot"].as_i64(),
                Some(mean(total(best))),
                "{pick}"
            );
        } else {
            assert_eq!(
                serde_json::to_value(why.line()).expect("json"),
                json!(format!(
                    "{}% sure",
                    (pick["share"]["bot"].as_u64().expect("share") + 5) / 10
                )),
                "{pick}"
            );
        }
        eprintln!("  pick at {at} checked: {} / {}", pick["you"], pick["bot"]);
    }
}

fn main() {
    let out = std::env::var("OUT").unwrap_or_else(|_| "target/web46/runs".to_owned());
    let player = Bot::champion(champion_search(1, 1, 2));
    let mut wanted: Vec<(&str, u64, fn(&Value, usize) -> bool)> = vec![
        ("win", 40, |end, _| end["end"] == "victory"),
        ("early", 6, |end, floor| {
            end["end"] == "defeat" && floor <= 10
        }),
        ("late", 20, |end, floor| {
            end["end"] == "defeat" && floor >= 25
        }),
    ];
    for k in 0..400 {
        if wanted.is_empty() {
            break;
        }
        let i = k % wanted.len();
        let (name, noise, fits) = wanted[i];
        let character = k % 5;
        let seed = format!("REVIEW{k}");
        let game = play(&player, character, &seed, noise);
        let s = game.run().state();
        let (floor, hp, max_hp) = (s.total_floor(), s.hp, s.max_hp);
        let end = match game.run().pending_decision() {
            sts2_core::run::RunDecision::Victory => json!({ "end": "victory" }),
            sts2_core::run::RunDecision::GameOver => json!({ "end": "defeat" }),
            _ => json!({ "end": "stopped" }),
        };
        eprintln!(
            "{name}? {seed} hero {character}: {end} floor {floor} ({} actions)",
            game.actions().len()
        );
        if !fits(&end, floor) {
            continue;
        }
        wanted.remove(i);
        let actions = game.actions();
        let save = json!({ "character": character, "seed": seed, "ascension": 0, "modifiers": [], "actions": actions, "end": end["end"], "floor": floor, "hp": hp, "max_hp": max_hp });
        std::fs::write(format!("{out}/{name}.json"), save.to_string()).expect("write");
        let (state, result) = review(character, &seed, actions);
        let mut ranks: Vec<(i64, bool)> = state["found"]
            .as_array()
            .expect("found")
            .iter()
            .map(|f| (f["rank"].as_i64().unwrap_or(0), f["kind"] == "combat"))
            .collect();
        ranks.sort_unstable();
        ranks.reverse();
        let of = |fight: bool| {
            ranks
                .iter()
                .filter(|r| r.1 == fight)
                .map(|r| r.0)
                .take(12)
                .collect::<Vec<_>>()
        };
        eprintln!(
            "  seen {}; fight gaps {:?}; choice gaps {:?}",
            state["seen"],
            of(true),
            of(false)
        );
        check_picks(character, &seed, actions, &result);
        std::fs::write(
            format!("{out}/{name}.review.json"),
            serde_json::to_string_pretty(&result).expect("json"),
        )
        .expect("write");
        println!(
            "{name}: {}",
            serde_json::to_string_pretty(&result["picks"]).expect("json")
        );
    }
}
