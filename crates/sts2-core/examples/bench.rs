//! Combats per second: the starter deck against `FuzzyWurmCrawlerWeak` with a uniformly random
//! policy over the legal actions, one fresh seed per combat.
//!
//! `cargo run --release -p sts2-core --example bench -- [COMBATS]`

use std::time::Instant;

use sts2_core::{Combat, CombatConfig, Status};

/// `xorshift64*`: a tiny deterministic policy RNG (not game logic).
struct Policy(u64);

impl Policy {
    fn pick(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let x = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        usize::try_from(x % u64::try_from(n).expect("fits u64")).expect("below n")
    }
}

#[allow(clippy::print_stdout, reason = "the example's output is its purpose")]
fn main() {
    let combats: u32 = std::env::args()
        .nth(1)
        .map_or(2000, |n| n.parse().expect("COMBATS is a number"));
    let mut policy = Policy(0x9E37_79B9_7F4A_7C15);
    let mut steps = 0u64;
    let mut won = 0u32;
    let start = Instant::now();
    for i in 0..combats {
        let mut combat = Combat::new(CombatConfig::necrobinder(&format!("BENCH{i}"), 0));
        while matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
            let mut actions = combat.legal_actions();
            let action = actions.swap_remove(policy.pick(actions.len()));
            combat.apply(action).expect("legal action");
            steps += 1;
        }
        won += u32::from(combat.status() == Status::Won);
    }
    let micros = start.elapsed().as_micros().max(1);
    println!(
        "{combats} combats ({won} won), {steps} steps in {} ms: {} combats/s, {} steps/s",
        micros / 1000,
        u128::from(combats) * 1_000_000 / micros,
        u128::from(steps) * 1_000_000 / micros,
    );
}
