//! `RelicKind::status`: what the icon of a relic shows (`Status` of `Models.Relics/*.cs`, set
//! by hooks in the game and derived from the relic's fields here).

use super::*;
use crate::content::relics::RelicStatus::{Active, Disabled, Normal};
use crate::content::relics::{
    OwnedRelic, RelicData, RelicStatus, belt_buckle, bone_tea, brilliant_scarf, ember_tea,
    galactic_dust, happy_flower, iron_club, joss_paper, kunai, lava_rock, lizard_tail, maw_bank,
    metronome, nunchaku, paels_tooth, pen_nib, pendulum, pumpkin_candle, rainbow_ring,
    silken_tress, silver_crucible, tea_of_discourtesy, toy_box, tuning_fork, venerable_tea_set,
    winged_boots, wongos_mystery_ticket,
};
use crate::run::{Act1, Run, RunState, UnlockState};

fn run_state() -> RunState {
    Run::new(
        Character::Necrobinder,
        "T6S0",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("T6S0 starts")
    .state()
    .clone()
}

/// A fight with `kind` (default data) as relic 1, on its first turn.
fn fight(kind: RelicKind) -> Combat {
    let mut config = CombatConfig::necrobinder(SEED, 0);
    config.relics.push(OwnedRelic {
        kind,
        data: kind.init_data(),
        melted: false,
    });
    Combat::new(config)
}

fn set_data(c: &mut Combat, data: RelicData) {
    c.edit(|s| s.relic_mut(RelicId(1)).data = data);
}

fn in_fight(c: &Combat) -> RelicStatus {
    let relic = c.state.relic(RelicId(1));
    relic.kind.status(&relic.data, Some(&c.state), &run_state())
}

/// The status out of a fight.
fn idle(kind: RelicKind, data: RelicData) -> RelicStatus {
    kind.status(&data, None, &run_state())
}

#[test]
fn counted_relics_pulse_one_before_their_trigger() {
    // `Status = X == Cards - 1 ? Active : Normal`, kept between fights (no `AfterCombatEnd`).
    let pen = |attacks_played| {
        RelicData::PenNib(pen_nib::Data {
            attacks_played,
            attack_to_double: None,
        })
    };
    assert_eq!(idle(RelicKind::PenNib, pen(9)), Active);
    assert_eq!(idle(RelicKind::PenNib, pen(8)), Normal);
    let nunchaku = |attacks_played| RelicData::Nunchaku(nunchaku::Data { attacks_played });
    assert_eq!(idle(RelicKind::Nunchaku, nunchaku(9)), Active);
    assert_eq!(idle(RelicKind::Nunchaku, nunchaku(0)), Normal);
    let club = |cards_played| RelicData::IronClub(iron_club::Data { cards_played });
    assert_eq!(idle(RelicKind::IronClub, club(3)), Active);
    assert_eq!(idle(RelicKind::IronClub, club(4)), Normal);
    let fork = |skills_played| RelicData::TuningFork(tuning_fork::Data { skills_played });
    assert_eq!(idle(RelicKind::TuningFork, fork(9)), Active);
    assert_eq!(idle(RelicKind::TuningFork, fork(3)), Normal);
    let paper = |cards_exhausted| {
        RelicData::JossPaper(joss_paper::Data {
            cards_exhausted,
            ethereal_count: 0,
        })
    };
    assert_eq!(idle(RelicKind::JossPaper, paper(4)), Active);
    assert_eq!(idle(RelicKind::JossPaper, paper(0)), Normal);
    let dust = |stars_spent| RelicData::GalacticDust(galactic_dust::Data { stars_spent });
    assert_eq!(idle(RelicKind::GalacticDust, dust(9)), Active);
    assert_eq!(idle(RelicKind::GalacticDust, dust(0)), Normal);
    let tea = |gain_energy_in_next_combat| {
        RelicData::VenerableTeaSet(venerable_tea_set::Data {
            gain_energy_in_next_combat,
        })
    };
    assert_eq!(idle(RelicKind::VenerableTeaSet, tea(true)), Active);
    assert_eq!(idle(RelicKind::VenerableTeaSet, tea(false)), Normal);
    assert_eq!(idle(RelicKind::FakeVenerableTeaSet, tea(true)), Active);
}

#[test]
fn spent_relics_are_disabled() {
    let tea = |kind, combats_left| match kind {
        RelicKind::EmberTea => RelicData::EmberTea(ember_tea::Data { combats_left }),
        RelicKind::BoneTea => RelicData::BoneTea(bone_tea::Data { combats_left }),
        _ => RelicData::TeaOfDiscourtesy(tea_of_discourtesy::Data { combats_left }),
    };
    for kind in [
        RelicKind::EmberTea,
        RelicKind::BoneTea,
        RelicKind::TeaOfDiscourtesy,
    ] {
        assert_eq!(idle(kind, tea(kind, 0)), Disabled, "{kind:?}");
        assert_eq!(idle(kind, tea(kind, 1)), Normal, "{kind:?}");
    }
    let boots = |times_used| RelicData::WingedBoots(winged_boots::Data { times_used });
    assert_eq!(idle(RelicKind::WingedBoots, boots(3)), Disabled);
    assert_eq!(idle(RelicKind::WingedBoots, boots(2)), Normal);
    let tail = |was_used| RelicData::LizardTail(lizard_tail::Data { was_used });
    assert_eq!(idle(RelicKind::LizardTail, tail(true)), Disabled);
    assert_eq!(idle(RelicKind::LizardTail, tail(false)), Normal);
    let bank = |has_item_been_bought| {
        RelicData::MawBank(maw_bank::Data {
            has_item_been_bought,
        })
    };
    assert_eq!(idle(RelicKind::MawBank, bank(true)), Disabled);
    assert_eq!(idle(RelicKind::MawBank, bank(false)), Normal);
    let rock = |has_triggered| RelicData::LavaRock(lava_rock::Data { has_triggered });
    assert_eq!(idle(RelicKind::LavaRock, rock(true)), Disabled);
    assert_eq!(idle(RelicKind::LavaRock, rock(false)), Normal);
    let tress = |is_used| RelicData::SilkenTress(silken_tress::Data { is_used });
    assert_eq!(idle(RelicKind::SilkenTress, tress(true)), Disabled);
    assert_eq!(idle(RelicKind::SilkenTress, tress(false)), Normal);
    let ticket = |gave_relic| {
        RelicData::WongosMysteryTicket(wongos_mystery_ticket::Data {
            combats_finished: 5,
            gave_relic,
        })
    };
    assert_eq!(idle(RelicKind::WongosMysteryTicket, ticket(true)), Disabled);
    assert_eq!(idle(RelicKind::WongosMysteryTicket, ticket(false)), Normal);
    let toy = |combats_seen| RelicData::ToyBox(toy_box::Data { combats_seen });
    assert_eq!(idle(RelicKind::ToyBox, toy(15)), Disabled);
    assert_eq!(idle(RelicKind::ToyBox, toy(14)), Normal);
    let crucible = |times_used, treasure_rooms_entered| {
        RelicData::SilverCrucible(silver_crucible::Data {
            times_used,
            treasure_rooms_entered,
        })
    };
    assert_eq!(idle(RelicKind::SilverCrucible, crucible(3, 1)), Disabled);
    assert_eq!(idle(RelicKind::SilverCrucible, crucible(3, 0)), Normal);
    assert_eq!(idle(RelicKind::SilverCrucible, crucible(2, 1)), Normal);
    let candle = |kindle_count| RelicData::PumpkinCandle(pumpkin_candle::Data { kindle_count });
    assert_eq!(idle(RelicKind::PumpkinCandle, candle(0)), Disabled);
    assert_eq!(idle(RelicKind::PumpkinCandle, candle(5)), Normal);
    let mut tooth = paels_tooth::Data::default();
    assert_eq!(
        idle(RelicKind::PaelsTooth, RelicData::PaelsTooth(tooth)),
        Disabled
    );
    tooth.cards[0] = Some(crate::run::RunCard::new("STRIKE_NECROBINDER"));
    assert_eq!(
        idle(RelicKind::PaelsTooth, RelicData::PaelsTooth(tooth)),
        Normal
    );
}

#[test]
fn pantograph_pulses_before_the_boss() {
    let mut s = run_state();
    let relic = RelicKind::Pantograph;
    assert_eq!(relic.status(&RelicData::None, None, &s), Normal);
    let parent = s.map.boss().parents.iter().next().expect("a boss parent");
    s.visited.push(parent);
    assert_eq!(relic.status(&RelicData::None, None, &s), Active);
}

#[test]
fn trigger_once_relics_pulse_until_used() {
    // Active from the start of the fight (`BeforeCombatStart`/`AfterRoomEntered`) until used.
    for kind in [
        RelicKind::UnsettlingLamp,
        RelicKind::Vambrace,
        RelicKind::ThrowingAxe,
        RelicKind::BurningSticks,
    ] {
        let c = fight(kind);
        assert_eq!(in_fight(&c), Active, "{kind:?}");
        let relic = c.state.relic(RelicId(1));
        assert_eq!(
            kind.status(&relic.data, None, &run_state()),
            Normal,
            "{kind:?} outside"
        );
    }
    let mut c = fight(RelicKind::ThrowingAxe);
    set_data(
        &mut c,
        RelicData::ThrowingAxe(crate::content::relics::throwing_axe::Data {
            used_this_combat: true,
        }),
    );
    assert_eq!(in_fight(&c), Normal);
    let mut c = fight(RelicKind::BurningSticks);
    set_data(
        &mut c,
        RelicData::BurningSticks(crate::content::relics::burning_sticks::Data {
            was_used_this_combat: true,
        }),
    );
    assert_eq!(in_fight(&c), Normal);
    let mut c = fight(RelicKind::UnsettlingLamp);
    set_data(
        &mut c,
        RelicData::UnsettlingLamp(crate::content::relics::unsettling_lamp::Data {
            is_finished_triggering: true,
            ..Default::default()
        }),
    );
    assert_eq!(in_fight(&c), Normal);
}

#[test]
fn hp_relics_follow_the_half_line() {
    let set_hp = |c: &mut Combat, hp: i32| {
        c.edit(|s| {
            let p = s.player.creature;
            s.creatures[p.index()].hp = hp;
        });
    };
    for kind in [RelicKind::RedSkull, RelicKind::MeatOnTheBone] {
        let mut c = fight(kind);
        let max = c.state.creature(c.state.player.creature).max_hp;
        set_hp(&mut c, max);
        assert_eq!(in_fight(&c), Normal, "{kind:?} at full HP");
        set_hp(&mut c, max / 2);
        assert_eq!(in_fight(&c), Active, "{kind:?} at half");
        set_hp(&mut c, max / 2 + 1);
        assert_eq!(in_fight(&c), Normal, "{kind:?} just above half");
    }
}

#[test]
fn belt_buckle_pulses_without_potions() {
    let mut c = fight(RelicKind::BeltBuckle);
    assert_eq!(in_fight(&c), Active);
    c.edit(|s| s.player.unported_potions.push(Some("FIRE_POTION")));
    assert_eq!(in_fight(&c), Normal);
    let data = RelicData::BeltBuckle(belt_buckle::Data::default());
    assert_eq!(idle(RelicKind::BeltBuckle, data), Normal);
}

fn var(kind: RelicKind, name: &str) -> i32 {
    kind.canonical_vars()
        .iter()
        .find(|v| v.name == name)
        .and_then(|v| v.value)
        .expect("a var")
        .to_int_truncating()
}

#[test]
fn turn_counters_pulse_on_the_turn_before_the_trigger() {
    for kind in [
        RelicKind::HappyFlower,
        RelicKind::FakeHappyFlower,
        RelicKind::Pendulum,
        RelicKind::PollinousCore,
    ] {
        let turns = var(kind, "Turns");
        let seen = |turns_seen| match kind {
            RelicKind::Pendulum => RelicData::Pendulum(pendulum::Data { turns_seen }),
            RelicKind::PollinousCore => {
                RelicData::PollinousCore(crate::content::relics::pollinous_core::Data {
                    turns_seen,
                })
            }
            _ => RelicData::HappyFlower(happy_flower::Data { turns_seen }),
        };
        let mut c = fight(kind);
        set_data(&mut c, seen(turns - 1));
        assert_eq!(in_fight(&c), Active, "{kind:?}");
        set_data(&mut c, seen(0));
        assert_eq!(in_fight(&c), Normal, "{kind:?}");
        assert_eq!(
            idle(kind, seen(turns - 1)),
            Normal,
            "{kind:?} outside a fight"
        );
    }
}

#[test]
fn per_turn_counters_pulse_in_a_fight() {
    let kunai = |attacks_played_this_turn| {
        RelicData::Kunai(kunai::Data {
            attacks_played_this_turn,
        })
    };
    let mut c = fight(RelicKind::Kunai);
    set_data(&mut c, kunai(2));
    assert_eq!(in_fight(&c), Active);
    set_data(&mut c, kunai(0));
    assert_eq!(in_fight(&c), Normal);
    assert_eq!(idle(RelicKind::Kunai, kunai(2)), Normal);

    let mut c = fight(RelicKind::RainbowRing);
    let ring = |activations| {
        RelicData::RainbowRing(rainbow_ring::Data {
            activations,
            ..Default::default()
        })
    };
    set_data(&mut c, ring(1));
    assert_eq!(in_fight(&c), Active);
    set_data(&mut c, ring(0));
    assert_eq!(in_fight(&c), Normal);

    let mut c = fight(RelicKind::Metronome);
    let orbs = var(RelicKind::Metronome, "OrbCount");
    let metronome = |orbs_channeled| RelicData::Metronome(metronome::Data { orbs_channeled });
    set_data(&mut c, metronome(orbs - 1));
    assert_eq!(in_fight(&c), Active);
    set_data(&mut c, metronome(0));
    assert_eq!(in_fight(&c), Normal);

    let mut c = fight(RelicKind::BrilliantScarf);
    let scarf = |cards_played_this_turn| {
        RelicData::BrilliantScarf(brilliant_scarf::Data {
            cards_played_this_turn,
        })
    };
    set_data(&mut c, scarf(4));
    assert_eq!(in_fight(&c), Active);
    set_data(&mut c, scarf(3));
    assert_eq!(in_fight(&c), Normal);

    let mut c = fight(RelicKind::Pocketwatch);
    assert_eq!(in_fight(&c), Active);
    set_data(
        &mut c,
        RelicData::Pocketwatch(crate::content::relics::pocketwatch::Data {
            cards_played_this_turn: 4,
            cards_played_last_turn: 0,
        }),
    );
    assert_eq!(in_fight(&c), Normal);
}

#[test]
fn turn_gated_relics_pulse_from_their_turn() {
    let mut c = fight(RelicKind::PaelsFlesh);
    assert_eq!(in_fight(&c), Normal);
    c.edit(|s| s.player.turn_number = 3);
    assert_eq!(in_fight(&c), Active);

    let mut c = fight(RelicKind::StoneCalendar);
    assert_eq!(in_fight(&c), Normal);
    c.edit(|s| s.player.turn_number = 7);
    assert_eq!(in_fight(&c), Active);
    c.edit(|s| s.side = Side::Enemy);
    assert_eq!(in_fight(&c), Normal, "its turn is over");

    let mut c = fight(RelicKind::PaelsLegion);
    assert_eq!(in_fight(&c), Active, "no cooldown");
    set_data(
        &mut c,
        RelicData::PaelsLegion(crate::content::relics::paels_legion::Data {
            cooldown: 2,
            ..Default::default()
        }),
    );
    assert_eq!(in_fight(&c), Normal);

    let mut c = fight(RelicKind::EmotionChip);
    assert_eq!(in_fight(&c), Normal);
    let hit = |turn| {
        RelicData::EmotionChip(crate::content::relics::emotion_chip::Data {
            last_unblocked_hit_turn: Some(turn),
        })
    };
    set_data(&mut c, hit(1));
    assert_eq!(in_fight(&c), Active);
    c.edit(|s| s.player.turn_number = 2);
    assert_eq!(in_fight(&c), Normal, "the next turn start cleared it");
}

#[test]
fn first_play_of_a_turn_ends_the_pulse() {
    for kind in [
        RelicKind::RippleBasin,
        RelicKind::ArtOfWar,
        RelicKind::PaelsEye,
    ] {
        let mut c = fight(kind);
        assert_eq!(in_fight(&c), Active, "{kind:?} at turn start");
        let target = enemy(&c);
        let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
        c.apply(Action::PlayCard {
            card: strike,
            target: Some(target),
        })
        .expect("legal");
        assert_eq!(in_fight(&c), Normal, "{kind:?} after an Attack");
    }
    let mut c = fight(RelicKind::PaelsEye);
    set_data(
        &mut c,
        RelicData::PaelsEye(crate::content::relics::paels_eye::Data {
            used_this_combat: true,
            ..Default::default()
        }),
    );
    assert_eq!(in_fight(&c), Normal);
}
