//! The enemy-turn preview ([`Combat::incoming`]), the powers and cards an intent names, and the
//! partial outcome of a play that draws (CQ1).

use super::*;
use crate::content::relics::{OwnedRelic, RelicData, centennial_puzzle, lizard_tail};

/// A fight against `encounter`, with or without Osty.
fn fight(encounter: Encounter, keep_osty: bool) -> Combat {
    let mut c = Combat::new(
        CombatConfig::necrobinder(SEED, 0)
            .with_encounter(encounter)
            .with_events(),
    );
    if !keep_osty {
        let osty = osty(&c);
        with_engine(&mut c, |eng| eng.kill(&[osty]));
    }
    c
}

/// A fight against `encounter` without Osty, the player owning `relic` as well.
fn fight_with_relic(encounter: Encounter, kind: RelicKind, data: RelicData) -> Combat {
    let mut config = CombatConfig::necrobinder(SEED, 0)
        .with_encounter(encounter)
        .with_events();
    config.relics.push(OwnedRelic {
        kind,
        data,
        melted: false,
    });
    let mut c = Combat::new(config);
    let osty = osty(&c);
    with_engine(&mut c, |eng| eng.kill(&[osty]));
    c
}

fn give_power(c: &mut Combat, kind: PowerKind, amount: i32) {
    let p = player(c);
    with_engine(c, |eng| {
        eng.apply_power(p, kind, Decimal::from(amount), Some(p))
    });
}

/// The preview, checked against the turn really ending: the player's HP when the next turn
/// starts is the preview's.
fn checked_incoming(c: &Combat) -> IncomingPreview {
    let preview = c.incoming().expect("the player's turn");
    let mut real = c.clone();
    real.apply(Action::EndTurn).expect("legal");
    let p = player(c);
    assert_eq!(real.state.creature(p).hp, preview.hp_after, "{preview:?}");
    assert_eq!(real.status() == Status::Lost, preview.lethal, "{preview:?}");
    preview
}

#[test]
fn incoming_spends_block_hit_by_hit() {
    // VineShambler's opening move hits twice for 6 (CreatureCmd.Damage per hit: Block first).
    let mut c = fight(Encounter::VineShamblerNormal, false);
    let p = player(&c);
    c.state.creature_mut(p).block = 8;
    let hp = c.state.creature(p).hp;
    let preview = checked_incoming(&c);
    assert_eq!(preview.enemies.len(), 1);
    assert_eq!(preview.enemies[0].hits, [6, 6]);
    assert_eq!(preview.total, 12);
    assert_eq!(preview.blocked, 8);
    assert_eq!(preview.hp_loss, 4);
    assert_eq!(preview.enemies[0].hp_loss, 4);
    assert_eq!(preview.hp_after, hp - 4);
    assert!(preview.certain && !preview.lethal);
}

#[test]
fn incoming_hits_osty_first() {
    // DieForYouPower redirects unblocked damage to Osty; the overkill goes back to the player
    // (CreatureCmd.cs:296-311). Osty with 10 HP takes 6, then 4 of the next 6.
    let mut c = fight(Encounter::VineShamblerNormal, true);
    let o = osty(&c);
    let osty = c.state.creature_mut(o);
    (osty.hp, osty.max_hp) = (10, 10);
    let preview = checked_incoming(&c);
    assert_eq!(preview.absorbed_by, [(o, 10)]);
    assert_eq!(preview.hp_loss, 2);
}

#[test]
fn intangible_caps_each_hit() {
    // IntangiblePower.ModifyHpLostAfterOsty: each hit takes at most 1.
    let mut c = fight(Encounter::VineShamblerNormal, false);
    give_power(&mut c, PowerKind::Intangible, 2);
    let preview = checked_incoming(&c);
    assert_eq!(preview.hp_loss, 2);
}

#[test]
fn buffer_eats_a_hit() {
    // BufferPower.ModifyHpLostAfterOsty: the first hit that would lose HP loses none.
    let mut c = fight(Encounter::VineShamblerNormal, false);
    give_power(&mut c, PowerKind::Buffer, 1);
    let preview = checked_incoming(&c);
    assert_eq!(preview.hp_loss, 6);
}

#[test]
fn lethal_unless_a_revive_fires() {
    let mut c = fight(Encounter::VineShamblerNormal, false);
    let p = player(&c);
    c.state.creature_mut(p).hp = 10;
    let preview = checked_incoming(&c);
    assert!(preview.lethal && preview.certain);
    assert_eq!(preview.hp_after, 0);

    // LizardTail.ShouldDieLate: the save heals instead.
    let mut c = fight_with_relic(
        Encounter::VineShamblerNormal,
        RelicKind::LizardTail,
        RelicData::LizardTail(lizard_tail::Data::default()),
    );
    c.state.creature_mut(p).hp = 10;
    let preview = checked_incoming(&c);
    assert!(!preview.lethal);
    assert!(preview.hp_after > 0);
}

#[test]
fn a_draw_in_the_enemy_turn_makes_it_uncertain() {
    // CentennialPuzzle draws 3 the first time the player loses HP: the preview stops there, so
    // the first hit counts and the second is left out.
    let c = fight_with_relic(
        Encounter::VineShamblerNormal,
        RelicKind::CentennialPuzzle,
        RelicData::CentennialPuzzle(centennial_puzzle::Data::default()),
    );
    let preview = c.incoming().expect("the player's turn");
    assert!(!preview.certain);
    assert_eq!(preview.total, 12);
    assert_eq!(preview.hp_loss, 6);
}

#[test]
fn incoming_is_none_off_turn_and_sums_every_enemy() {
    let c = fight(Encounter::InkletsNormal, false);
    let preview = checked_incoming(&c);
    let hits: Vec<Vec<i32>> = preview.enemies.iter().map(|e| e.hits.clone()).collect();
    assert_eq!(hits, [vec![3], vec![2, 2, 2], vec![3]]);
    assert_eq!(preview.total, 12);
    assert_eq!(
        preview.enemies.iter().map(|e| e.hp_loss).sum::<i32>(),
        preview.hp_loss
    );
    let mut over = c.clone();
    let p = player(&c);
    over.state.creature_mut(p).hp = 1;
    over.apply(Action::EndTurn).expect("legal");
    assert_eq!(over.incoming(), None);
}

#[test]
fn intents_name_their_powers_and_cards() {
    // Axebot's opener: an attack and a debuff of Weak 2 and Frail 2 on the player.
    let c = fight(Encounter::AxebotsNormal, true);
    let intents = c.state.intents(enemy(&c));
    let debuff = intents
        .iter()
        .find(|i| i.kind == IntentType::Debuff)
        .expect("a debuff intent");
    assert_eq!(
        debuff.applies,
        [(PowerKind::Weak, 2), (PowerKind::Frail, 2)]
    );
    let preview = c.incoming().expect("the player's turn");
    assert_eq!(preview.enemies[0].applies, debuff.applies);

    // A Chomper that screams first adds Dazed.
    let c = fight(Encounter::ChompersNormal, true);
    let screamer = c.state.enemies[1];
    let intents = c.state.intents(screamer);
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].kind, IntentType::StatusCard);
    assert_eq!(intents[0].card, Some(CardKind::Dazed));
    let count = intents[0].cards.expect("a status intent's count");
    let preview = c.incoming().expect("the player's turn");
    assert_eq!(preview.enemies[1].status_cards, [(CardKind::Dazed, count)]);
}

#[test]
fn a_play_that_draws_gives_its_damage_first() {
    // Pommel Strike deals its damage, then draws: the outcome stops at the draw.
    let mut c = fight(Encounter::VineShamblerNormal, true);
    let target = enemy(&c);
    let hp = c.state.creature(target).hp;
    let card = give_card(&mut c, CardKind::PommelStrike, 0);
    assert_eq!(c.play_outcome(card, Some(target)), None, "it draws");
    let outcome = c
        .play_outcome_until_draw(card, Some(target))
        .expect("certain up to the draw");
    assert!(outcome.partial && !outcome.wins);
    let o = outcome.of(target).expect("the target is hit");
    assert_eq!((o.hp_before, o.hp_after), (hp, hp - 9));
    // A play with no draw is whole.
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let outcome = c.play_outcome(strike, Some(target)).expect("certain");
    assert!(!outcome.partial);
    assert_eq!(
        c.play_outcome_until_draw(strike, Some(target)),
        Some(outcome)
    );
}

#[test]
fn killing_a_creature_that_left_the_combat_does_nothing() {
    // CreatureCmd.KillWithoutCheckingWinCondition returns first for a creature whose
    // CombatState is null (RemoveCreature unattached it).
    let mut c = fight(Encounter::InkletsNormal, true);
    let inklet = enemy(&c);
    with_engine(&mut c, |eng| eng.kill(&[inklet]));
    assert!(
        !c.state.contains_creature(inklet),
        "a dead Inklet is removed"
    );
    let (state, events) = (c.state.clone(), c.events().len());
    with_engine(&mut c, |eng| eng.kill(&[inklet]));
    assert_eq!(c.events().len(), events);
    assert_eq!(c.state, state);
}

/// Every action `apply` could be handed now: each hand card and potion slot (one past the
/// belt too) at no target, every creature id and one past them, each pick of any card id, and
/// ending the turn.
fn every_action(c: &Combat) -> Vec<Action> {
    let s = &c.state;
    let targets: Vec<Option<CreatureId>> = std::iter::once(None)
        .chain((0..=s.creatures.len()).map(|i| Some(CreatureId(u32::try_from(i).expect("small")))))
        .collect();
    let mut actions = vec![Action::EndTurn, Action::Resolve(Decision::Done)];
    for &card in &s.player.hand {
        actions.extend(
            targets
                .iter()
                .map(|&target| Action::PlayCard { card, target }),
        );
    }
    for slot in 0..=s.player.potions.len() {
        actions.extend(
            targets
                .iter()
                .map(|&target| Action::UsePotion { slot, target }),
        );
    }
    for card in &s.cards {
        actions.push(Action::Resolve(Decision::Pick(card.id)));
    }
    actions
}

#[test]
fn apply_accepts_exactly_the_legal_actions() {
    let mut rng = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = |n: usize| {
        rng = rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(rng >> 33).expect("fits") % n
    };
    for (i, &encounter) in Encounter::ALL.iter().enumerate() {
        let mut config = CombatConfig::necrobinder(SEED, 0).with_encounter(encounter);
        config.potions = (0..3)
            .map(|k| Some(PotionKind::ALL[(i * 3 + k) % PotionKind::ALL.len()].into()))
            .collect();
        let mut c = Combat::new(config);
        for _ in 0..15 {
            let legal = c.legal_actions();
            if legal.is_empty() {
                break;
            }
            let me = c.state.player.creature;
            for action in every_action(&c) {
                let accepted = c.clone().apply(action.clone()).is_ok();
                // `EnqueueManualUse`: a potion used at no target is used on its owner if valid.
                let listed = legal.contains(&action)
                    || matches!(action, Action::UsePotion { slot, target: None }
                        if legal.contains(&Action::UsePotion { slot, target: Some(me) }));
                assert_eq!(accepted, listed, "{encounter:?}: {action:?}");
            }
            let pick = legal[next(legal.len())].clone();
            c.apply(pick).expect("legal");
        }
    }
}

#[test]
fn content_asks_hook_queries_through_the_engine_list() {
    // The `CombatState` versions rebuild the whole listener list on every call; the engine's
    // `hooked()` reads the cached one (core-content-run #2).
    const QUERIES: [&str; 13] = [
        "hittable_enemies",
        "resolve_energy_x_value",
        "resolve_star_x_value",
        "energy_cost",
        "energy_to_spend",
        "star_cost",
        "max_energy",
        "keywords",
        "has_keyword",
        "should_retain_this_turn",
        "is_sly_this_turn",
        "costs_energy_or_stars",
        "unplayable_reason",
    ];
    fn walk(dir: &std::path::Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("readable") {
            let path = entry.expect("readable").path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).expect("readable");
                for (i, line) in text.lines().enumerate() {
                    if QUERIES
                        .iter()
                        .any(|q| line.contains(&format!("eng.s.{q}(")))
                    {
                        found.push(format!("{}:{}", path.display(), i + 1));
                    }
                }
            }
        }
    }
    let mut found = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/content"),
        &mut found,
    );
    assert!(found.is_empty(), "use `eng.hooked()` at {found:#?}");
}
