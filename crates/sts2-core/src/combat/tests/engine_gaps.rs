//! Spawns, secondary enemies, afflictions, enchantments, card and potion commands, gold and
//! per-instance power data (C6).

use super::*;
use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::content::afflictions::AfflictionKind;
use crate::content::cards::the_scythe;
use crate::content::cards::{CardData, CardKeyword, TargetType};
use crate::content::enchantments::{EnchantmentKind, EnchantmentStatus};
use crate::content::monsters::{self, EyeWithTeeth};

fn facing(encounter: Encounter) -> Combat {
    Combat::new(CombatConfig::necrobinder(SEED, 0).with_encounter(encounter))
}

/// Fogmog's Illusion move, run directly: the eye joins in the `illusion` slot.
fn add_eye(c: &mut Combat) -> CreatureId {
    with_engine(c, |eng| {
        eng.add_enemy(MonsterKind::EyeWithTeeth(EyeWithTeeth), Some("illusion"))
    })
}

#[test]
fn a_spawned_illusion_sorts_first_and_is_secondary() {
    // CreatureCmd.cs:38-80, CombatState.cs:496-502, EyeWithTeeth.cs, IllusionPower.cs.
    let mut c = facing(Encounter::FogmogNormal);
    let fogmog = enemy(&c);
    let eye = add_eye(&mut c);
    assert_eq!(c.state.enemies, [eye, fogmog]);
    let eye_c = c.state.creature(eye);
    assert_eq!((eye_c.hp, eye_c.slot), (6, Some("illusion")));
    assert!(eye_c.has_power(PowerKind::Illusion) && eye_c.has_power(PowerKind::Minion));
    assert!(eye_c.is_secondary_enemy() && c.state.creature(fogmog).is_primary_enemy());
    assert_eq!(
        eye_c.monster.as_ref().map(Monster::next_move_name),
        Some("DISTRACT_MOVE")
    );

    // CreatureCmd.cs:562-573: the last primary enemy takes its secondary teammates along.
    with_engine(&mut c, |eng| eng.kill(&[fogmog]));
    assert!(c.state.creature(eye).is_dead());
    assert!(c.state.is_ending());
}

#[test]
fn a_killed_illusion_stays_unhittable_and_revives_on_its_turn() {
    // IllusionPower.cs: AfterDeath forces REVIVE_MOVE (must be performed once), the owner
    // stays in combat and cannot be hit; the revive heals it to full.
    let mut c = facing(Encounter::FogmogNormal);
    let eye = add_eye(&mut c);
    with_engine(&mut c, |eng| eng.kill(&[eye]));
    assert!(c.state.enemies.contains(&eye) && c.state.creature(eye).is_dead());
    assert!(!c.state.is_ending(), "Fogmog is still alive");
    assert!(!c.state.hittable_enemies().contains(&eye));
    let next = |c: &Combat| {
        c.state
            .creature(eye)
            .monster
            .as_ref()
            .map(Monster::next_move_name)
    };
    assert_eq!(next(&c), Some("REVIVE_MOVE"));
    with_engine(&mut c, |eng| monsters::perform_move(eng, eye));
    assert_eq!(c.state.creature(eye).hp, 6);
    assert!(c.state.hittable_enemies().contains(&eye));
    with_engine(&mut c, |eng| {
        monsters::roll_move(eng, eye);
        Ok(())
    });
    assert_eq!(next(&c), Some("DISTRACT_MOVE"));
}

#[test]
fn tangled_entangles_attacks_and_raises_their_cost() {
    // TangledPower.cs, CardCmd.cs:617-673.
    let mut c = new_combat();
    let p = player(&c);
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let defend = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Tangled, Decimal::ONE, None)
    });
    let entangled = |c: &Combat, card| c.state.card(card).affliction.map(|a| (a.kind, a.amount));
    assert_eq!(entangled(&c, strike), Some((AfflictionKind::Entangled, 1)));
    assert_eq!(entangled(&c, defend), None);
    assert_eq!(c.state.energy_cost(strike), 2);
    assert_eq!(c.state.energy_cost(defend), 1);
    assert!(
        c.state
            .history_this_turn(HistoryTag::CardAfflicted)
            .iter()
            .any(|e| e.kind
                == HistoryKind::CardAfflicted {
                    card: strike,
                    affliction: AfflictionKind::Entangled,
                })
    );

    // A new Attack entering combat is afflicted too (AfterCardEnteredCombat).
    let new = with_engine(&mut c, |eng| {
        let id = eng.create_card(CardKind::StrikeNecrobinder);
        eng.add_to_pile(id, PileType::Hand, Position::Bottom)?;
        Ok(id)
    });
    assert_eq!(entangled(&c, new), Some((AfflictionKind::Entangled, 1)));

    // Removal clears every Entangled.
    let tangled = c
        .state
        .creature(p)
        .powers
        .iter()
        .find(|p| p.kind == PowerKind::Tangled)
        .map(|p| p.id);
    with_engine(&mut c, |eng| eng.remove_power(p, tangled.expect("applied")));
    assert_eq!(entangled(&c, strike), None);
    assert_eq!(c.state.energy_cost(strike), 1);
}

#[test]
fn vigor_boosts_one_attack_and_spends_its_amount() {
    // VigorPower.cs: the remembered attack gets +Amount, then Amount drops by that much.
    let mut c = new_combat();
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Vigor, Decimal::from(3), None)
    });
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let hp = c.state.creature(e).hp;
    c.apply(Action::PlayCard {
        card: strike,
        target: Some(e),
    })
    .expect("legal");
    assert_eq!(c.state.creature(e).hp, hp - 9);
    assert!(!c.state.creature(p).has_power(PowerKind::Vigor));
}

#[test]
fn vigorous_adds_damage_until_its_card_is_played() {
    // Vigorous.cs, Hook.cs:1499-1509 (the enchantment runs before the listeners).
    let mut c = new_combat();
    let e = enemy(&c);
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    with_engine(&mut c, |eng| {
        eng.enchant(strike, EnchantmentKind::Vigorous, Decimal::from(2));
        Ok(())
    });
    let hp = c.state.creature(e).hp;
    c.apply(Action::PlayCard {
        card: strike,
        target: Some(e),
    })
    .expect("legal");
    assert_eq!(c.state.creature(e).hp, hp - 8);
    let status = c.state.card(strike).enchantment.map(|e| e.status);
    assert_eq!(status, Some(EnchantmentStatus::Disabled));
}

#[test]
fn slither_rerolls_the_cost_when_drawn() {
    // Slither.cs: AfterCardDrawn of its own card sets the cost for the combat on
    // Rng.CombatEnergyCosts.
    let mut c = new_combat();
    let card = c.state.player.draw[0];
    with_engine(&mut c, |eng| {
        eng.enchant(card, EnchantmentKind::Slither, Decimal::ONE);
        Ok(())
    });
    let mut rng = c.state.rng[RunRngType::CombatEnergyCosts].clone();
    let expected = rng.next_int(4);
    with_engine(&mut c, |eng| eng.draw(Decimal::ONE, false));
    assert_eq!(c.state.energy_cost(card), expected);
    assert_eq!(c.state.rng[RunRngType::CombatEnergyCosts], rng);
}

#[test]
fn downgrade_keeps_the_scythes_increase() {
    // CardModel.cs:2142-2156, TheScythe.cs.
    let mut c = new_combat();
    let e = enemy(&c);
    c.state.player.energy = 9;
    let scythe = give_card(&mut c, CardKind::TheScythe, 1);
    c.apply(Action::PlayCard {
        card: scythe,
        target: Some(e),
    })
    .expect("legal");
    let data = c.state.card(scythe).data;
    with_engine(&mut c, |eng| {
        eng.downgrade(scythe);
        Ok(())
    });
    let card = c.state.card(scythe);
    assert_eq!(card.upgrade_level, 0);
    assert_eq!(card.data, data);
    assert_eq!(
        card.data,
        CardData::TheScythe(the_scythe::Data {
            increased_damage: 7,
            current_damage: 20,
        })
    );
}

#[test]
fn a_discarded_sly_card_is_autoplayed() {
    // CardCmd.cs:172-209: discard, CardDiscarded entry, then autoplay the Sly cards.
    let mut c = new_combat();
    let e = enemy(&c);
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    c.state.card_mut(strike).keywords.insert(CardKeyword::Sly);
    let hp = c.state.creature(e).hp;
    with_engine(&mut c, |eng| eng.discard(&[strike]));
    assert!(
        c.state
            .history_this_turn(HistoryTag::CardDiscarded)
            .iter()
            .any(|h| h.kind == HistoryKind::CardDiscarded(strike))
    );
    assert_eq!(c.state.creature(e).hp, hp - 6);
    assert_eq!(c.state.pile_of(strike), Some(PileType::Discard));
}

#[test]
fn transform_replaces_the_card_in_place() {
    // CardCmd.cs:374-520.
    let mut c = new_combat();
    let original = c.state.player.draw[1];
    let soul = with_engine(&mut c, |eng| {
        eng.transform_to(original, CardKind::Soul)?;
        Ok(eng.s.player.draw[1])
    });
    assert_eq!(c.state.card(soul).kind, CardKind::Soul);
    assert_eq!(c.state.pile_of(original), None);
}

#[test]
fn potions_are_procured_into_the_first_free_slot_and_discarded() {
    // PotionCmd.cs, Player.cs:628-706.
    let mut c = with_potions(&[PotionKind::FirePotion]);
    let added = with_engine(&mut c, |eng| {
        eng.try_to_procure_potion(PotionKind::BlockPotion)
    });
    assert!(added);
    assert_eq!(
        belt(&c),
        [
            Some(PotionKind::FirePotion),
            Some(PotionKind::BlockPotion),
            None
        ]
    );
    with_engine(&mut c, |eng| eng.discard_potion(0));
    assert_eq!(belt(&c), [None, Some(PotionKind::BlockPotion), None]);
    with_engine(&mut c, |eng| {
        eng.try_to_procure_potion(PotionKind::FirePotion)?;
        eng.try_to_procure_potion(PotionKind::FirePotion)
    });
    let full = with_engine(&mut c, |eng| {
        eng.try_to_procure_potion(PotionKind::FirePotion)
    });
    assert!(!full);
}

#[test]
fn gold_is_gained_and_lost_down_to_zero() {
    // PlayerCmd.cs:143-206.
    let mut c = new_combat();
    with_engine(&mut c, |eng| eng.gain_gold(Decimal::from(10)));
    assert_eq!(c.state.player.gold, 109);
    with_engine(&mut c, |eng| {
        eng.lose_gold(Decimal::from(500));
        Ok(())
    });
    assert_eq!(c.state.player.gold, 0);
}

#[test]
fn a_random_position_draws_a_shuffle_index_even_for_an_empty_pile() {
    // CardPileCmd.cs:509: `Rng.Shuffle.NextInt(cardPile.Cards.Count + 1)` per added card.
    let mut c = new_combat();
    c.state.player.draw.clear();
    let mut rng = c.state.rng[RunRngType::Shuffle].clone();
    let souls: Vec<CardId> = with_engine(&mut c, |eng| {
        let souls: Vec<CardId> = (0..3).map(|_| eng.create_card(CardKind::Soul)).collect();
        eng.add_generated_cards_to_combat(&souls, PileType::Draw, None, Position::Random)?;
        Ok(souls)
    });
    let mut expected = Vec::new();
    for (&soul, bound) in souls.iter().zip([1, 2, 3]) {
        expected.insert(
            usize::try_from(rng.next_int(bound)).expect("non-negative"),
            soul,
        );
    }
    assert_eq!(c.state.rng[RunRngType::Shuffle], rng);
    assert_eq!(*c.state.player.draw, expected);
}

#[test]
fn replay_count_and_energy_value_reach_the_card_play() {
    // CardModel.cs:2031 (`GetEnchantedReplayCount() + 1`), PlayCardAction.cs:92-100.
    let mut c = new_combat();
    let e = enemy(&c);
    c.state.player.energy = 9;
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    c.state.card_mut(strike).base_replay_count = 1;
    let hp = c.state.creature(e).hp;
    c.apply(Action::PlayCard {
        card: strike,
        target: Some(e),
    })
    .expect("legal");
    assert_eq!(c.state.creature(e).hp, hp - 12);
    assert_eq!(c.state.player.energy, 8);
}

#[test]
fn the_scythe_starts_from_its_deck_version_and_buffs_it() {
    // Player.cs:806-814 (DeckVersion), TheScythe.cs (BuffFromPlay on both).
    let mut config = CombatConfig::necrobinder(SEED, 0);
    config.deck[0] = DeckCard {
        counter: 10,
        ..DeckCard::new(CardKind::TheScythe)
    };
    let mut c = Combat::new(config);
    let scythe = CardId(0);
    assert_eq!(c.state.card(scythe).deck_version(), Some(0));
    assert_eq!(
        c.state.card(scythe).data,
        CardData::TheScythe(the_scythe::Data {
            increased_damage: 10,
            current_damage: 23,
        })
    );
    let clone = with_engine(&mut c, |eng| Ok(eng.create_clone(scythe)));
    assert_eq!(c.state.card(clone).deck_version(), None);
    let e = enemy(&c);
    c.state.player.energy = 9;
    with_engine(&mut c, |eng| {
        eng.add_to_pile(scythe, PileType::Hand, Position::Bottom)
    });
    let hp = c.state.creature(e).hp;
    c.apply(Action::PlayCard {
        card: scythe,
        target: Some(e),
    })
    .expect("legal");
    assert_eq!(c.state.creature(e).hp, hp - 23);
    assert_eq!(c.state.deck[0].counter, 15);
}

#[test]
fn the_combat_knows_its_room_type() {
    // CombatRoom.cs:29: `RoomType => Encounter.RoomType`.
    assert_eq!(
        facing(Encounter::SlimesWeak).state.room_type,
        RoomType::Monster
    );
    assert_eq!(
        facing(Encounter::ByrdonisElite).state.room_type,
        RoomType::Elite
    );
    assert_eq!(
        facing(Encounter::VantomBoss).state.room_type,
        RoomType::Boss
    );
}

#[test]
fn a_random_attack_picks_one_living_enemy_per_hit() {
    // AttackCommand.cs:603-618: one `CombatTargets.NextItem` per hit over the living opponents.
    let mut c = facing(Encounter::InkletsNormal);
    let before = c.state.rng[RunRngType::CombatTargets].counter();
    let player = c.state.player.creature;
    let results = with_engine(&mut c, |eng| {
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::RandomOpponents,
                AttackDamage::Fixed(Decimal::ONE),
                None,
            )
            .with_hits(3),
        )
    });
    assert_eq!(results.len(), 3);
    assert_eq!(c.state.rng[RunRngType::CombatTargets].counter(), before + 3);
}

/// Enchants `card` and plays it at `target`.
fn enchant_and_play(c: &mut Combat, card: CardId, kind: EnchantmentKind, amount: i32) {
    with_engine(c, |eng| {
        eng.enchant(card, kind, Decimal::from(amount));
        Ok(())
    });
    let target = (c.state.card(card).kind.target() == TargetType::AnyEnemy).then(|| enemy(c));
    c.apply(Action::PlayCard { card, target }).expect("legal");
}

#[test]
fn adroit_gains_block_after_the_card_and_nimble_adds_to_it() {
    // Adroit.cs (OnPlay, CardModel.cs:1940), Hook.cs:1326-1330 (before every block listener).
    let mut c = new_combat();
    let p = player(&c);
    c.state.player.energy = 9;
    let defend = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    enchant_and_play(&mut c, defend, EnchantmentKind::Nimble, 2);
    assert_eq!(c.state.creature(p).block, 7);

    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    enchant_and_play(&mut c, strike, EnchantmentKind::Adroit, 3);
    assert_eq!(c.state.creature(p).block, 10);
}

#[test]
fn sown_and_swift_act_on_the_first_play_only() {
    // Sown.cs, Swift.cs.
    let mut c = new_combat();
    c.state.player.energy = 9;
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    enchant_and_play(&mut c, strike, EnchantmentKind::Sown, 2);
    assert_eq!(c.state.player.energy, 9 - 1 + 2);
    let status = c.state.card(strike).enchantment.map(|e| e.status);
    assert_eq!(status, Some(EnchantmentStatus::Disabled));

    let hand = c.state.player.hand.len();
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    enchant_and_play(&mut c, strike, EnchantmentKind::Swift, 2);
    assert_eq!(c.state.player.hand.len(), hand + 1 - 1 + 2);
}

#[test]
fn momentum_adds_its_amount_to_the_next_play() {
    // Momentum.cs: OnPlay raises `_extraDamage`, which `EnchantDamageAdditive` adds.
    let mut c = new_combat();
    let e = enemy(&c);
    c.state.player.energy = 9;
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let hp = c.state.creature(e).hp;
    enchant_and_play(&mut c, strike, EnchantmentKind::Momentum, 2);
    assert_eq!(c.state.creature(e).hp, hp - 6);
    assert_eq!(
        c.state.card(strike).enchantment.map(|e| e.extra_damage),
        Some(2)
    );
}

#[test]
fn royally_approved_adds_innate_and_retain_and_a_downgrade_restores_them() {
    // RoyallyApproved.cs (OnEnchant), CardModel.cs:2159 (DowngradeInternal runs ModifyCard).
    let mut c = new_combat();
    let defend = give_card(&mut c, CardKind::DefendNecrobinder, 1);
    with_engine(&mut c, |eng| {
        eng.enchant(defend, EnchantmentKind::RoyallyApproved, Decimal::ONE);
        Ok(())
    });
    let keywords = c.state.card(defend).keywords;
    assert!(keywords.contains(CardKeyword::Innate) && keywords.contains(CardKeyword::Retain));
    with_engine(&mut c, |eng| {
        eng.downgrade(defend);
        Ok(())
    });
    let keywords = c.state.card(defend).keywords;
    assert!(keywords.contains(CardKeyword::Innate) && keywords.contains(CardKeyword::Retain));
}

/// Regression (RLB9, seed 1095028): a step that auto-plays a card which opens its own choice
/// panicked with "no model in combat reports `may_prompt`" when the auto-player did not report it.
#[test]
fn auto_play_models_may_prompt() {
    use crate::content::cards::CardKind;
    use crate::content::potions::PotionKind;
    use crate::content::powers::PowerKind;
    use crate::content::relics::RelicKind;
    for card in [
        CardKind::Cascade,
        CardKind::Uproar,
        CardKind::IAmInvincible,
        CardKind::BeatDown,
        CardKind::Eidolon,
        CardKind::Havoc,
        CardKind::KnifeTrap,
    ] {
        assert!(card.may_prompt(), "{card:?}");
    }
    assert!(PowerKind::Stampede.may_prompt());
    assert!(PowerKind::Hellraiser.may_prompt());
    assert!(RelicKind::HistoryCourse.may_prompt());
    assert!(RelicKind::WhisperingEarring.may_prompt());
    assert!(PotionKind::DistilledChaos.may_prompt());
}
