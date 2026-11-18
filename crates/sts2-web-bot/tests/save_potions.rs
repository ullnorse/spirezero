//! WEB84: "Net potions: Save for elites & bosses": in a hallway fight the net's potion uses are
//! not candidates; elite and boss fights are unchanged.
//!
//! The run is played by the bot outside fights and by the net inside them, from the start, on
//! a few seeds until both kinds of position are met. The weights are local data
//! (`~/.cache/spirebot-data/net/cb3.bin`); the test says so and passes when they are missing.
//! Run it with `--release`. `WEB84_SAVE=1` prints the two positions as the page's saves, to load
//! in the browser.

#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a test that reports what it found"
)]

use std::path::PathBuf;

use sts2_core::combat::Action;
use sts2_core::run::{Room, RoomType, RunAction, RunDecision};
use sts2_web::Game;
use sts2_web_bot::fights::{CombatNet, Pick};
use sts2_web_bot::{bot, choose};

const ASCENSION: u8 = 0;
const CHARACTER: usize = 4;

const fn potion(pick: &Pick) -> bool {
    matches!(pick.action, RunAction::Combat(Action::UsePotion { .. }))
}

const fn room(game: &Game) -> Option<RoomType> {
    match game.run().state().room {
        Room::Combat { room_type, .. } => Some(room_type),
        _ => None,
    }
}

fn print_save(seed: &str, actions: &[u32], kind: &str) {
    if std::env::var_os("WEB84_SAVE").is_some() {
        println!(
            "SAVE {kind} {}",
            serde_json::json!({
                "character": CHARACTER, "seed": seed, "ascension": ASCENSION,
                "modifiers": [], "actions": actions,
            })
        );
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "whole runs take minutes unoptimized: cargo test --release -p sts2-web-bot"
)]
fn potions_are_saved_in_hallway_fights_only() {
    let home = std::env::var_os("HOME").expect("HOME");
    let path = PathBuf::from(home).join(".cache/spirebot-data/net/cb3.bin");
    let Ok(weights) = std::fs::read(&path) else {
        println!("save_potions: no local weights ({path:?}): skipped");
        return;
    };
    let mut net = CombatNet::from_weights(&weights).unwrap();
    let (mut hallway, mut elite) = (None, None);
    for seed in 1..=40u32 {
        let name = format!("SAVE{seed}");
        let mut game = Game::new(CHARACTER, &name, ASCENSION, &[]).unwrap();
        for _ in 0..4000 {
            if game.is_over() {
                break;
            }
            let fight = matches!(game.run().pending_decision(), RunDecision::Combat(_));
            let action = if fight {
                let acts = game.actions().to_vec();
                let any = net.pick(CHARACTER, &name, ASCENSION, &[], &acts).unwrap();
                let saved = net
                    .pick_with(CHARACTER, &name, ASCENSION, &[], &acts, true)
                    .unwrap();
                if let (Some(any), Some(saved)) = (&any, &saved) {
                    match room(&game) {
                        Some(RoomType::Monster) => {
                            assert!(!potion(saved), "seed {seed}: a potion in a hallway fight");
                            if potion(any) && hallway.is_none() {
                                println!("hallway: seed {seed}, {} actions", acts.len());
                                print_save(&name, &acts, "hallway");
                                hallway = Some((any.clone(), saved.clone()));
                            }
                        }
                        Some(RoomType::Elite | RoomType::Boss) => {
                            assert_eq!(any, saved, "seed {seed}: the elite pick changed");
                            if potion(any) && elite.is_none() {
                                println!("elite: seed {seed}, {} actions", acts.len());
                                print_save(&name, &acts, "elite");
                                elite = Some(any.clone());
                            }
                        }
                        _ => {}
                    }
                }
                any.map(|p| p.action)
            } else {
                choose(bot(), &game).map(|(a, _)| a)
            };
            let Some(action) = action else { break };
            let index = game
                .recordable_actions()
                .iter()
                .position(|a| *a == action)
                .expect("recordable");
            game.advance(&[u32::try_from(index).unwrap()]).unwrap();
        }
        if hallway.is_some() && elite.is_some() {
            break;
        }
    }
    let (any, saved) = hallway.expect("no hallway position where the net picks a potion");
    assert!(potion(&any) && !potion(&saved));
    assert!(
        elite.is_some(),
        "no elite position where the net picks a potion"
    );
}
