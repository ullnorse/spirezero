//! The event section (RLN33, `sts2_rl::obs_events`): the specs name real content and vars,
//! hand-checked options give what their text says, hidden state never changes a vector, and
//! the share of options with a full vector per event.

use std::collections::BTreeMap;

use sts2_core::content::ancients::AncientOption;
use sts2_core::content::events::EventKind;
use sts2_core::rng::Rng;
use sts2_core::run::{Run, RunAction, RunDecision};
use sts2_rl::RunConfig;
use sts2_rl::obs_events::{
    EVENTS_AT, EVENTS_BYTES, OPTION_BYTES, OptionVector, ancient_option, check_specs, event_option,
};
use sts2_rl::run_obs::{RUN_OBS_BYTES, RunObserver};
use sts2_rl::vocab;

/// A run on `seed` past Neow (its first option), on the map.
fn on_map(seed: u64) -> Run {
    let mut run = RunConfig::default().run(seed).expect("a run");
    while !matches!(run.pending_decision(), RunDecision::Map(_)) {
        let legal = run.legal_actions();
        run.apply(legal[0].clone()).expect("a legal action");
    }
    run
}

/// `kind`'s first page on `seed`, as (text key, vector) per option.
fn page(kind: EventKind, seed: u64) -> Vec<(String, OptionVector)> {
    let mut run = on_map(seed);
    run.debug_enter_event(kind).expect("the event begins");
    vectors(&run)
}

fn vectors(run: &Run) -> Vec<(String, OptionVector)> {
    let RunDecision::Event(room) = run.pending_decision() else {
        panic!("not on an event page");
    };
    room.options
        .iter()
        .map(|o| (o.text_key.clone(), event_option(room, run.state(), o)))
        .collect()
}

fn option(page: &[(String, OptionVector)], suffix: &str) -> OptionVector {
    page.iter()
        .find(|(k, _)| k.ends_with(suffix))
        .unwrap_or_else(|| panic!("no option {suffix} in {page:?}"))
        .1
}

#[test]
fn every_spec_names_real_content_and_declared_vars() {
    let bad = check_specs();
    assert!(bad.is_empty(), "{bad:#?}");
}

/// One option per effect kind, against the game's text (`localization_eng/events.json`,
/// `relics.json`) and the vars the text shows.
#[test]
fn hand_checked_options_give_what_their_text_says() {
    // "Gain {MaxHp} Max HP. Take {Damage} damage." (2, 3; another page follows) and "Heal
    // {Heal} HP." (10).
    let abyssal = page(EventKind::AbyssalBaths, 1);
    let immerse = option(&abyssal, "IMMERSE");
    assert!(immerse.known());
    assert_eq!(
        (
            immerse.get("max_hp"),
            immerse.get("hp"),
            immerse.get("more")
        ),
        (2, -3, 1)
    );
    assert_eq!(option(&abyssal, "ABSTAIN").get("hp"), 10);

    // "Add {Card} to your Deck." (Byrdonis Egg) and "Gain {MaxHp} Max HP." (7).
    let nest = page(EventKind::ByrdonisNest, 1);
    let take = option(&nest, "TAKE");
    assert_eq!(
        (take.get("cards"), take.id("card")),
        (1, vocab::card_id("BYRDONIS_EGG"))
    );
    assert_eq!(option(&nest, "EAT").get("max_hp"), 7);

    // "Obtain {Relic}. Lose {HpLoss} Max HP." (Fresnel Lens, 13) and "Procure {Potion}."
    let beacon = page(EventKind::DrowningBeacon, 1);
    let climb = option(&beacon, "CLIMB");
    assert_eq!(
        (climb.get("relics"), climb.id("relic"), climb.get("max_hp")),
        (1, vocab::relic_id("FRESNEL_LENS"), -13)
    );
    let bottle = option(&beacon, "BOTTLE");
    assert_eq!(
        (bottle.get("potions"), bottle.id("potion")),
        (1, vocab::potion_id("GLOWWATER_POTION"))
    );

    // "Remove 1 card from your Deck. Add {BatheCurses} Guilty to your Deck." and "Procure 1
    // random Potion."
    let spring = page(EventKind::Wellspring, 1);
    let bathe = option(&spring, "BATHE");
    assert_eq!(
        (
            bathe.get("remove"),
            bathe.get("choose"),
            bathe.get("curses"),
            bathe.id("curse")
        ),
        (1, 1, 1, vocab::card_id("GUILTY"))
    );
    let potion = option(&spring, "BOTTLE");
    assert_eq!((potion.get("potions"), potion.get("random")), (1, 1));

    // "Pay {BreathingTechniquesCost} Gold. Add 2 Enlightenment to your Deck." (50).
    let weaver = page(EventKind::ZenWeaver, 1);
    let breathe = option(&weaver, "BREATHING_TECHNIQUES");
    assert!(breathe.known());
    assert_eq!(
        (
            breathe.get("gold"),
            breathe.get("cards"),
            breathe.id("card")
        ),
        (-50, 2, vocab::card_id("ENLIGHTENMENT"))
    );

    // "Transform a card in your Deck." and "Upgrade a card in your Deck."
    let aroma = page(EventKind::AromaOfChaos, 1);
    assert_eq!(
        (
            option(&aroma, "LET_GO").get("transform"),
            option(&aroma, "MAINTAIN_CONTROL").get("upgrade")
        ),
        (1, 1)
    );

    // "Enchant a card with {Enchantment}." (Sown).
    let seed = page(EventKind::SapphireSeed, 1);
    let plant = option(&seed, "PLANT");
    assert_eq!(plant.get("enchant"), 1);
    assert_ne!(plant.ench, 0);
    assert_eq!(option(&seed, "EAT").get("hp"), 9);

    // Ancient options are relics with their pickup effects: Golden Pearl "Upon pickup, gain
    // {Gold} Gold.", Lead Paperweight "choose 1 of 2 Colorless cards", Precarious Shears
    // "remove {Cards} cards ... and lose {Damage} HP".
    let pearl = ancient_option(AncientOption::relic("GOLDEN_PEARL"));
    assert!(pearl.known());
    assert_eq!(
        (pearl.get("relics"), pearl.id("relic")),
        (1, vocab::relic_id("GOLDEN_PEARL"))
    );
    assert!(pearl.get("gold") > 0);
    let paperweight = ancient_option(AncientOption::relic("LEAD_PAPERWEIGHT"));
    assert_eq!(
        (paperweight.get("cards"), paperweight.get("choices")),
        (1, 2)
    );
    let shears = ancient_option(AncientOption::relic("PRECARIOUS_SHEARS"));
    assert!(shears.known() && shears.get("remove") > 0 && shears.get("hp") < 0);
    // A relic whose effect is the relic itself.
    let sozu = ancient_option(AncientOption::relic("SOZU"));
    assert!(sozu.known());
    assert_eq!(sozu.columns.iter().sum::<i32>(), 2, "known and relics only");
}

/// Two runs that differ only in what the player cannot know (the event's own `Rng`, the run's
/// and the player's streams) give the same vectors: every event's first page, on many seeds.
#[test]
fn hidden_rolls_do_not_change_a_vector() {
    let mut pages = 0;
    for kind in EventKind::ALL {
        for seed in 0..6 {
            let mut run = on_map(seed);
            if run.debug_enter_event(kind).is_err() {
                continue;
            }
            let RunDecision::Event(room) = run.pending_decision() else {
                continue;
            };
            if room.options.is_empty() {
                continue;
            }
            let mut hidden_room = room.clone();
            hidden_room.rng = Rng::new(seed ^ 0xE7E7);
            let other = on_map(seed + 500);
            let mut hidden = run.state().clone();
            hidden.rng.clone_from(&other.state().rng);
            hidden.player_rng.clone_from(&other.state().player_rng);
            let observe = |s, decision: &RunDecision<'_>| {
                let mut out = vec![0; RUN_OBS_BYTES];
                RunObserver::new(true)
                    .with_events(true)
                    .observe_parts(s, decision, None, &mut out);
                out[EVENTS_AT..EVENTS_AT + EVENTS_BYTES].to_vec()
            };
            let base = observe(run.state(), &RunDecision::Event(room));
            assert!(base.iter().any(|&b| b != 0), "{kind:?}: an empty section");
            assert_eq!(
                base,
                observe(&hidden, &RunDecision::Event(&hidden_room)),
                "{kind:?} seed {seed}"
            );
            pages += 1;
        }
    }
    assert!(pages > 200, "{pages} pages");
}

/// The section is zero with the flag off, and the rest of the row is the same either way.
#[test]
fn off_writes_nothing_and_on_changes_nothing_else() {
    let mut run = on_map(3);
    run.debug_enter_event(EventKind::Wellspring)
        .expect("the event begins");
    let decision = run.pending_decision();
    let mut off = vec![0xAA; RUN_OBS_BYTES];
    let mut on = vec![0xAA; RUN_OBS_BYTES];
    RunObserver::new(true).observe_parts(run.state(), &decision, None, &mut off);
    RunObserver::new(true)
        .with_events(true)
        .observe_parts(run.state(), &decision, None, &mut on);
    assert!(off[EVENTS_AT..].iter().all(|&b| b == 0));
    assert_eq!(off[..EVENTS_AT], on[..EVENTS_AT]);
    assert!(
        on[EVENTS_AT..EVENTS_AT + 2 * OPTION_BYTES]
            .iter()
            .any(|&b| b != 0)
    );
}

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
}

/// Options with a full vector / options met, per event, walking every event's pages with random
/// choices on many seeds (and Neow's options): printed with `--nocapture`.
#[test]
fn coverage_per_event() {
    let mut per: BTreeMap<String, (BTreeMap<String, bool>, u32)> = BTreeMap::new();
    let mut policy = Policy(33);
    for kind in EventKind::ALL {
        for seed in 0..12 {
            let mut run = on_map(seed);
            if run.debug_enter_event(kind).is_err() {
                continue;
            }
            for _ in 0..12 {
                if !matches!(run.pending_decision(), RunDecision::Event(_)) {
                    break;
                }
                let entry = per.entry(kind.id().to_owned()).or_default();
                for (key, v) in vectors(&run) {
                    let part = key.split(".options.").nth(1).unwrap_or(&key).to_owned();
                    *entry.0.entry(part).or_insert(true) &= v.known();
                }
                let legal: Vec<RunAction> = run
                    .legal_actions()
                    .into_iter()
                    .filter(|a| matches!(a, RunAction::EventOption(_)))
                    .collect();
                if legal.is_empty() {
                    break;
                }
                let nth = usize::try_from(policy.next() % legal.len() as u64).expect("fits");
                if run.apply(legal[nth].clone()).is_err() {
                    break;
                }
            }
        }
    }
    for seed in 0..200 {
        let run = RunConfig::default().run(seed).expect("a run");
        if let RunDecision::Ancient(room) = run.pending_decision() {
            let entry = per.entry(room.kind.id().to_owned()).or_default();
            for &o in &room.options {
                *entry.0.entry(o.name().to_owned()).or_insert(true) &= ancient_option(o).known();
            }
        }
    }
    let (mut full, mut all) = (0, 0);
    for (event, (options, _)) in &per {
        let n = options.values().filter(|&&k| k).count();
        let gaps: Vec<&String> = options
            .iter()
            .filter(|(_, k)| !**k)
            .map(|(o, _)| o)
            .collect();
        println!("{event}: {n}/{} {gaps:?}", options.len());
        full += n;
        all += options.len();
    }
    println!("total: {full}/{all}");
    assert!(per.len() > 40, "{} events", per.len());
    assert!(
        full * 100 >= all * 90,
        "{full}/{all} options with a full vector"
    );
}
