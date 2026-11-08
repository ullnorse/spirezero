//! The vocabularies are append-only and cover the engine's content: every pinned prefix of
//! a file (`vocab/pins.txt`) is unchanged, every line is pinned, and every kind the engine has
//! is listed. `STS2_VOCAB_APPEND=1 cargo test -p sts2-rl --test vocab` appends the missing
//! kinds and pins the new lines; it never touches a pinned line.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use sts2_core::combat::Encounter;
use sts2_core::content::ancients::AncientKind;
use sts2_core::content::cards::CardKind;
use sts2_core::content::events::EventKind;
use sts2_core::content::monsters::MonsterKind;
use sts2_core::content::potions::PotionKind;
use sts2_core::content::powers::PowerKind;
use sts2_core::content::relics::RelicKind;
use sts2_core::rng::RunRngSet;
use sts2_core::run::rooms::ALL_ACTS;
use sts2_rl::vocab::{self, FILES, FNV_BASIS, fnv};

/// The vocabulary directory.
fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vocab")
}

/// The hash a pin records: FNV-1a over the first lines, each with its newline.
fn prefix_hash(lines: &[&str]) -> u64 {
    lines
        .iter()
        .fold(FNV_BASIS, |h, line| fnv(fnv(h, line.as_bytes()), b"\n"))
}

/// A pin: a file, a line count and the hash of that many first lines.
struct Pin {
    file: String,
    lines: usize,
    hash: u64,
}

fn pins(text: &str) -> Vec<Pin> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            assert_eq!(parts.len(), 3, "a pin is `file lines hash`: {l}");
            Pin {
                file: parts[0].to_owned(),
                lines: parts[1].parse().expect("a line count"),
                hash: u64::from_str_radix(parts[2], 16).expect("a hex hash"),
            }
        })
        .collect()
}

/// The ids the engine has, per vocabulary file.
fn engine_ids(name: &str) -> Vec<&'static str> {
    match name {
        "cards.txt" => CardKind::ALL.iter().map(|k| k.id()).collect(),
        "relics.txt" => RelicKind::ALL.iter().map(|k| k.id()).collect(),
        "powers.txt" => PowerKind::ALL.iter().map(|k| k.id()).collect(),
        "potions.txt" => PotionKind::ALL.iter().map(|k| k.id()).collect(),
        "encounters.txt" => Encounter::ALL.iter().map(|k| k.id()).collect(),
        "events.txt" => EventKind::ALL.iter().map(|k| k.id()).collect(),
        "ancients.txt" => AncientKind::ALL.iter().map(|k| k.id()).collect(),
        "acts.txt" => ALL_ACTS.iter().map(|a| a.id).collect(),
        // Every monster model, summons included (`MonsterKind` holds data, so `IDS`).
        "monsters.txt" => MonsterKind::IDS.to_vec(),
        other => panic!("no engine ids for {other}"),
    }
}

#[test]
fn vocab_is_append_only_and_complete() {
    let append = std::env::var_os("STS2_VOCAB_APPEND").is_some();
    let pins_path = dir().join("pins.txt");
    let pins_text = fs::read_to_string(&pins_path).expect("vocab/pins.txt");
    let pins = pins(&pins_text);
    let mut new_pins = String::new();
    let mut failures = Vec::new();
    for file in FILES {
        let path = dir().join(file.name);
        let text = fs::read_to_string(&path).expect("a vocabulary file");
        let mut lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines.get(..2),
            Some(&["<pad>", "<unk>"][..]),
            "{} starts with <pad> and <unk>",
            file.name
        );
        let mut seen = HashSet::new();
        for line in &lines {
            assert!(!line.trim().is_empty(), "{} has a blank line", file.name);
            assert!(seen.insert(*line), "{} lists {line} twice", file.name);
        }
        // Existing ids keep their numbers.
        let mut pinned = 0;
        for pin in pins.iter().filter(|p| p.file == file.name) {
            assert!(
                pin.lines <= lines.len() && prefix_hash(&lines[..pin.lines]) == pin.hash,
                "{}: the first {} lines changed since they were pinned: an id was renumbered \
                 or removed (the vocabularies are append-only)",
                file.name,
                pin.lines
            );
            pinned = pinned.max(pin.lines);
        }
        let missing: Vec<&str> = engine_ids(file.name)
            .into_iter()
            .filter(|id| !seen.contains(id))
            .collect();
        if !missing.is_empty() {
            if append {
                let mut grown = text.clone();
                for id in &missing {
                    writeln!(grown, "{id}").expect("a string");
                }
                fs::write(&path, &grown).expect("append to the vocabulary");
                lines.extend(&missing);
            } else {
                failures.push(format!("{}: missing {missing:?}", file.name));
            }
        }
        if pinned < lines.len() {
            if append {
                writeln!(
                    new_pins,
                    "{} {} {:016x}",
                    file.name,
                    lines.len(),
                    prefix_hash(&lines)
                )
                .expect("a string");
            } else {
                failures.push(format!("{}: lines {pinned}.. are not pinned", file.name));
            }
        }
    }
    if append && !new_pins.is_empty() {
        fs::write(&pins_path, pins_text + &new_pins).expect("append the pins");
    }
    assert!(
        failures.is_empty(),
        "{}\n(run `STS2_VOCAB_APPEND=1 cargo test -p sts2-rl --test vocab` to append and pin)",
        failures.join("\n")
    );
}

#[test]
fn vocab_ids_are_the_line_numbers() {
    for kind in CardKind::ALL {
        assert_eq!(vocab::card(kind), id_of(&vocab::CARDS, kind.id()));
    }
    for kind in PowerKind::ALL {
        assert_eq!(vocab::power(kind), id_of(&vocab::POWERS, kind.id()));
    }
    for kind in RelicKind::ALL {
        assert_eq!(vocab::relic(kind), id_of(&vocab::RELICS, kind.id()));
    }
    for kind in PotionKind::ALL {
        assert_eq!(vocab::potion(kind), id_of(&vocab::POTIONS, kind.id()));
    }
    for kind in Encounter::ALL {
        assert_eq!(vocab::encounter(kind), id_of(&vocab::ENCOUNTERS, kind.id()));
    }
    for kind in EventKind::ALL {
        assert_eq!(vocab::event(kind), id_of(&vocab::EVENTS, kind.id()));
        assert_eq!(vocab::event_id(kind.id()), vocab::event(kind));
    }
    for kind in AncientKind::ALL {
        assert_eq!(vocab::ancient(kind), id_of(&vocab::ANCIENTS, kind.id()));
    }
    for act in ALL_ACTS {
        assert_eq!(vocab::act(act.id), id_of(&vocab::ACTS, act.id));
    }
    for kind in CardKind::ALL {
        assert_eq!(vocab::card_id(kind.id()), vocab::card(kind));
    }
    for kind in RelicKind::ALL {
        assert_eq!(vocab::relic_id(kind.id()), vocab::relic(kind));
    }
    for kind in PotionKind::ALL {
        assert_eq!(vocab::potion_id(kind.id()), vocab::potion(kind));
    }
    for kind in Encounter::ALL {
        assert_eq!(vocab::encounter_id(kind.id()), vocab::encounter(kind));
    }
    assert_eq!(vocab::card_id("NOT_A_CARD"), vocab::UNK);
    // The monsters every encounter spawns, over a few seeds, by their ordinal.
    for seed in 0..8 {
        let rng = RunRngSet::new(&format!("VOCAB{seed}"));
        for encounter in Encounter::ALL {
            for (monster, _) in encounter.generate_monsters(&rng, seed) {
                assert_eq!(MonsterKind::IDS[monster.ordinal()], monster.id());
                assert_eq!(
                    vocab::monster(&monster),
                    id_of(&vocab::MONSTERS, monster.id())
                );
                assert_eq!(vocab::monster_id(monster.id()), vocab::monster(&monster));
            }
        }
    }
    assert_eq!(vocab::monster_id("NOT_A_MONSTER"), vocab::UNK);
    assert_eq!(vocab::monster_id("<pad>"), vocab::UNK);
}

/// The line number of `id` in `file`.
fn id_of(file: &vocab::File, id: &str) -> u16 {
    let line = file.ids().position(|l| l == id).expect("listed");
    u16::try_from(line).expect("fits u16")
}
