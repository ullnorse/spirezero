//! Differential test: the kernel against sts2-core, step by step.
//!
//! Every fight starts from the same sts2-core combat. At every step both engines list their legal
//! actions (which must be the same list, in the same order), the policy picks one, both apply it,
//! and the kernel's state must equal the projection of sts2-core's ([`Fight::from_core`]), digest
//! included. The run streams the kernel does not carry must not move. Any mismatch is a kernel
//! bug. Release runs many more fights than debug builds (`cargo test -p sts2-kernel --release`).

use std::fmt::Write as _;

use sts2_core::combat::Encounter;
use sts2_core::rng::{RngState, RunRngType};
use sts2_core::run::rooms::{OVERGROWTH, UNDERDOCKS};
use sts2_core::{Character, Combat, CombatConfig, Status as CoreStatus};
use sts2_kernel::{Action, Fight, Forced, Kernel, P, Status};

/// Fights per (character, encounter) pair; `KERNEL_DIFF_FIGHTS` overrides it.
fn fights_per_pair() -> u64 {
    std::env::var("KERNEL_DIFF_FIGHTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) { 6 } else { 160 })
}
/// Actions per fight before the test gives up on it (no supported fight gets near).
const MAX_ACTIONS: u32 = 5_000;

/// `SplitMix64`.
struct Policy(u64);

impl Policy {
    const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).expect("fits")).expect("fits")
    }

    /// Uniform over `legal` in "random" mode; otherwise ends the turn only when nothing else
    /// is legal or one time in eight, which makes fights long (more shuffles, status cards and
    /// second phases).
    fn pick(&mut self, legal: &[Action], greedy: bool) -> usize {
        if !greedy || legal.len() == 1 || self.below(8) == 0 {
            return self.below(legal.len());
        }
        let plays = legal.iter().filter(|a| **a != Action::EndTurn).count();
        let nth = self.below(plays);
        legal
            .iter()
            .enumerate()
            .filter(|(_, a)| **a != Action::EndTurn)
            .nth(nth)
            .map(|(i, _)| i)
            .expect("a play")
    }
}

/// The run streams the kernel does not carry.
fn untracked(c: &Combat) -> Vec<RngState> {
    RunRngType::ALL
        .into_iter()
        .filter(|t| {
            !matches!(
                t,
                RunRngType::Shuffle | RunRngType::MonsterAi | RunRngType::Niche
            )
        })
        .map(|t| c.state().rng[t].state())
        .collect()
}

/// A compact, line-per-part dump of a fight.
fn dump(f: &Fight) -> Vec<String> {
    let mut out = vec![format!(
        "status {:?} energy {}/{} round {} turn {} enemy_side {} in_progress {} pending_loss {} \
         hooks {} played {} pending {} enemies {:?} n_cards {} relics {}",
        f.status,
        f.energy,
        f.max_energy,
        f.round,
        f.turn,
        f.enemy_side,
        f.in_progress,
        f.pending_loss,
        f.hooks_active,
        f.played_this_turn,
        f.pending,
        f.enemies(),
        f.n_cards,
        f.relics,
    )];
    out.push(format!(
        "rng shuffle {:?} ai {:?} niche {:?}",
        f.rng_shuffle, f.rng_ai, f.rng_niche
    ));
    for i in 0..f.n_creatures {
        let c = f.creature(i);
        let powers: Vec<String> = c
            .powers()
            .iter()
            .map(|p| {
                format!(
                    "{:?}={} (skip {} applier {} data {})",
                    p.kind, p.amount, p.skip_tick, p.applier, p.data
                )
            })
            .collect();
        out.push(format!(
            "creature {i}: hp {}/{} block {} slot {} powers [{}]",
            c.hp,
            c.max_hp,
            c.block,
            c.slot,
            powers.join(", ")
        ));
        if c.enemy {
            out.push(format!("creature {i} monster: {:?}", c.monster));
        }
    }
    for p in 0..5 {
        let cards: Vec<String> = f
            .pile_cards(p)
            .map(|c| format!("{c}:{}{:?}", f.card(c).spec, f.card(c).affliction))
            .collect();
        out.push(format!("pile {p}: {}", cards.join(" ")));
    }
    out
}

/// The parts where two fights differ.
fn diff(kernel: &Fight, core: &Fight) -> String {
    let (a, b) = (dump(kernel), dump(core));
    let mut out = String::new();
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i), b.get(i));
        if x != y {
            let _ = writeln!(out, "kernel: {}", x.map_or("<none>", String::as_str));
            let _ = writeln!(out, "core:   {}", y.map_or("<none>", String::as_str));
        }
    }
    if out.is_empty() {
        out.push_str("(only in fields the dump leaves out)\n");
    }
    out
}

/// What one fight did.
#[derive(Default)]
struct Outcome {
    actions: u32,
    won: bool,
    lost: bool,
    decisions: u32,
    unsupported: bool,
    seen: Seen,
}

/// The irregular paths a fight went through, so the test shows it exercised them.
#[derive(Clone, Copy, Debug, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag per path the test must have seen"
)]
struct Seen {
    stunned: bool,
    revive: bool,
    summon: bool,
    reshuffle: bool,
    artifact_used: bool,
    afflicted: bool,
    slow_counted: bool,
    player_debuffs: u32,
    enemy_powers: u32,
}

impl Seen {
    fn update(&mut self, f: &Fight, start: &Fight) {
        for i in 1..f.n_creatures {
            let c = f.creature(i);
            self.stunned |= c.monster.forced == Forced::Stunned;
            self.revive |= c.monster.forced == Forced::Revive;
            self.slow_counted |= c.powers().iter().any(|p| p.kind == P::Slow && p.data > 0);
            self.enemy_powers |= c.mask;
            if i < start.n_creatures && start.creature(i).has(P::Artifact) && !c.has(P::Artifact) {
                self.artifact_used = true;
            }
        }
        self.summon |= f.n_creatures > start.n_creatures;
        self.reshuffle |= f.rng_shuffle != start.rng_shuffle;
        self.afflicted |= f.cards[..usize::from(f.n_cards)]
            .iter()
            .any(|c| c.affliction != sts2_kernel::Affliction::None);
        self.player_debuffs |= f.player().mask;
    }

    const fn merge(&mut self, o: Self) {
        self.stunned |= o.stunned;
        self.revive |= o.revive;
        self.summon |= o.summon;
        self.reshuffle |= o.reshuffle;
        self.artifact_used |= o.artifact_used;
        self.afflicted |= o.afflicted;
        self.slow_counted |= o.slow_counted;
        self.player_debuffs |= o.player_debuffs;
        self.enemy_powers |= o.enemy_powers;
    }
}

/// Plays one fight on both engines; panics at the first difference.
fn play(character: Character, encounter: Encounter, ascension: u8, seed: u64) -> Outcome {
    let k = Kernel::get();
    let mut config = CombatConfig::new(character, &format!("PF6K{seed}"), ascension)
        .expect("starter cards are ported")
        .with_encounter(encounter);
    if seed % 4 == 3 {
        // A long fight: lots of HP, and some cards upgraded.
        config.player_hp = 400;
        config.player_max_hp = 400;
        for (i, card) in config.deck.iter_mut().enumerate() {
            if (seed >> (i % 8)) & 1 == 1 && card.kind.is_upgradable(0) {
                card.upgrade_level = 1;
            }
        }
    }
    let mut core = Combat::new(config);
    let what = format!("{character:?} {encounter:?} A{ascension} seed {seed}");
    let mut kernel = Fight::from_core(k, &core).unwrap_or_else(|u| panic!("{what}: {u:?}"));
    let streams = untracked(&core);
    let mut policy = Policy(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x5A5A);
    let greedy = seed.is_multiple_of(2);
    let mut out = Outcome::default();
    let start = kernel;
    while matches!(core.status(), CoreStatus::PlayerTurn | CoreStatus::Decision) {
        assert!(out.actions < MAX_ACTIONS, "{what}: too long");
        let core_legal: Vec<Action> = core
            .legal_actions()
            .iter()
            .map(|a| Action::from_core(a).unwrap_or_else(|| panic!("{what}: {a:?}")))
            .collect();
        let legal = kernel.legal_actions(k);
        assert_eq!(
            legal.as_slice(),
            core_legal.as_slice(),
            "{what}: legal actions after {} actions",
            out.actions
        );
        let action = legal.as_slice()[policy.pick(legal.as_slice(), greedy)];
        out.decisions += u32::from(matches!(action, Action::Pick(_)));
        core.apply(action.to_core())
            .unwrap_or_else(|e| panic!("{what}: sts2-core refused {action:?}: {e}"));
        let status = kernel
            .step(k, action)
            .unwrap_or_else(|_| panic!("{what}: the kernel refused {action:?}"));
        out.actions += 1;
        if status == Status::Unsupported {
            // A capacity ran out; sts2-core goes on, the kernel has stopped.
            out.unsupported = true;
            break;
        }
        let projected = Fight::from_core(k, &core)
            .unwrap_or_else(|u| panic!("{what}: after {action:?}, out of scope: {u:?}"));
        let ours = kernel.canonical();
        assert!(
            ours == projected,
            "{what}: state after action {} ({action:?}) differs:\n{}",
            out.actions,
            diff(&ours, &projected)
        );
        assert_eq!(kernel.digest(), projected.digest());
        assert_eq!(status, projected.status);
        assert_eq!(
            untracked(&core),
            streams,
            "{what}: an untracked stream moved"
        );
        out.seen.update(&kernel, &start);
    }
    out.won = core.status() == CoreStatus::Won;
    out.lost = core.status() == CoreStatus::Lost;
    out
}

fn encounters(act: &sts2_core::run::rooms::ActInfo) -> Vec<Encounter> {
    act.encounters
        .iter()
        .map(|e| Encounter::from_entry(e.id).expect("ported"))
        .collect()
}

/// The Underdocks encounters the kernel supports.
const UNDERDOCKS_SUPPORTED: [&str; 16] = [
    "CORPSE_SLUGS_NORMAL",
    "CORPSE_SLUGS_WEAK",
    "CULTISTS_NORMAL",
    "FOSSIL_STALKER_NORMAL",
    "HAUNTED_SHIP_NORMAL",
    "PUNCH_CONSTRUCT_NORMAL",
    "SEAPUNK_NORMAL",
    "SEAPUNK_WEAK",
    "SEWER_CLAM_NORMAL",
    "SLUDGE_SPINNER_WEAK",
    "TOADPOLES_WEAK",
    "PHANTASMAL_GARDENERS_ELITE",
    "SKULKING_COLONY_ELITE",
    "TERROR_EEL_ELITE",
    "LAGAVULIN_MATRIARCH_BOSS",
    "SOUL_FYSH_BOSS",
];

#[test]
fn every_supported_fight_matches_sts2_core() {
    let mut fights = 0;
    let mut actions = 0;
    let mut wins = 0;
    let mut decisions = 0;
    let mut losses = 0;
    let mut unsupported = 0;
    let mut paths = Seen::default();
    let mut supported = encounters(&OVERGROWTH);
    supported.extend(
        UNDERDOCKS_SUPPORTED
            .iter()
            .map(|id| Encounter::from_entry(id).expect("ported")),
    );
    for character in [Character::Ironclad, Character::Silent] {
        for (e, &encounter) in supported.iter().enumerate() {
            for i in 0..fights_per_pair() {
                let seed = 1_000_000 * u64::try_from(e).expect("fits") + i;
                let ascension = u8::try_from(seed % 11).expect("fits");
                let out = play(character, encounter, ascension, seed);
                fights += 1;
                actions += out.actions;
                wins += u32::from(out.won);
                decisions += out.decisions;
                losses += u32::from(out.lost);
                unsupported += u32::from(out.unsupported);
                paths.merge(out.seen);
            }
        }
    }
    eprintln!(
        "differential: {fights} fights, {actions} actions, {wins} won, {losses} lost, \
         {decisions} choices, {unsupported} stopped at a capacity, 0 mismatches; {paths:?}"
    );
    assert!(
        unsupported * 100 <= fights,
        "too many fights outgrew a capacity"
    );
    assert!(decisions > 0 && wins > 0 && losses > 0);
    assert!(paths.stunned && paths.revive && paths.summon && paths.reshuffle);
    assert!(paths.artifact_used && paths.afflicted && paths.slow_counted);
    for p in [
        P::Vulnerable,
        P::Weak,
        P::Frail,
        P::Shrink,
        P::Constrict,
        P::Tangled,
        P::Ringing,
    ] {
        assert!(
            paths.player_debuffs & p.bit() != 0,
            "no fight gave the player {p:?}"
        );
    }
}

#[test]
fn unsupported_content_is_refused_cleanly() {
    // Underdocks encounters the kernel does not support are refused by `from_core`, never
    // half-played.
    let k = Kernel::get();
    let mut refused = 0;
    for encounter in encounters(&UNDERDOCKS)
        .into_iter()
        .filter(|e| !UNDERDOCKS_SUPPORTED.contains(&e.entry()))
    {
        let config = CombatConfig::new(Character::Ironclad, "PF6U", 0)
            .expect("ported")
            .with_encounter(encounter);
        let core = Combat::new(config);
        if Fight::from_core(k, &core).is_err() {
            refused += 1;
        }
    }
    assert_eq!(
        refused,
        encounters(&UNDERDOCKS).len() - UNDERDOCKS_SUPPORTED.len()
    );
    // Other characters' starter cards are refused too.
    let config = CombatConfig::new(Character::Necrobinder, "PF6U", 0).expect("ported");
    assert!(Fight::from_core(k, &Combat::new(config)).is_err());
}

#[test]
fn illegal_actions_leave_the_fight_unchanged() {
    let k = Kernel::get();
    let config = CombatConfig::new(Character::Ironclad, "PF6I", 0).expect("ported");
    let core = Combat::new(config);
    let mut f = Fight::from_core(k, &core).expect("in scope");
    let before = f;
    assert!(f.step(k, Action::Pick(0)).is_err());
    assert!(
        f.step(
            k,
            Action::Play {
                card: 200,
                target: 0
            }
        )
        .is_err()
    );
    assert_eq!(f, before);
    assert_eq!(f.status, Status::PlayerTurn);
}
