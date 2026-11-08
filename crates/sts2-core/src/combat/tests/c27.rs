//! C27: the card preview (`CardModel.UpdateDynamicVarPreview`, `DamageVar.cs:28`,
//! `BlockVar.cs:26`): the numbers a card's text shows.

use super::*;
use crate::content::cards::{PreviewMode, Var};
use crate::content::enchantments::{Enchantment, EnchantmentKind};

fn preview(c: &Combat, card: CardId, target: Option<CreatureId>, var: Var) -> Decimal {
    c.state
        .card_preview(card, target, PreviewMode::Normal)
        .into_iter()
        .find(|&(v, _)| v == var)
        .expect("the card has the var")
        .1
}

#[test]
fn hand_cards_preview_the_damage_and_block_hooks() {
    // DamageVar.UpdateCardPreview: Hook.ModifyDamage with the player as dealer and the aimed
    // target; BlockVar: Hook.ModifyBlock on the player.
    let mut c = new_combat();
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Strength, Decimal::from(2), Some(p))?;
        eng.apply_power(p, PowerKind::Dexterity, Decimal::from(1), Some(p))?;
        eng.apply_power(e, PowerKind::Vulnerable, Decimal::from(2), Some(p))
    });
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let defend = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    assert_eq!(preview(&c, strike, None, Var::Damage), Decimal::from(8));
    assert_eq!(preview(&c, strike, Some(e), Var::Damage), Decimal::from(12));
    assert_eq!(preview(&c, defend, None, Var::Block), Decimal::from(6));
}

#[test]
fn cards_outside_the_hand_preview_only_their_enchantment() {
    // UpdateDynamicVarPreview: runGlobalHooks only for the Hand and Play piles.
    let mut c = new_combat();
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Strength, Decimal::from(2), Some(p))
    });
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    c.edit(|s| s.card_mut(strike).enchantment = Some(Enchantment::new(EnchantmentKind::Sharp, 3)));
    assert_eq!(preview(&c, strike, None, Var::Damage), Decimal::from(11));
    c.edit(|s| {
        s.remove_from_pile(strike);
        s.insert_into_pile(strike, PileType::Draw, None);
    });
    assert_eq!(preview(&c, strike, None, Var::Damage), Decimal::from(9));
    // The same card out of combat.
    let card = *c.state.card(strike);
    let out: Vec<_> = card.preview_out_of_combat().collect();
    assert_eq!(out, vec![(Var::Damage, Decimal::from(9))]);
}

#[test]
fn unpowered_vars_skip_strength() {
    // Burn: `DamageVar(2m, ValueProp.Unpowered | ValueProp.Move)`.
    let mut c = new_combat();
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Strength, Decimal::from(2), Some(p))
    });
    let burn = give_card(&mut c, CardKind::Burn, 0);
    assert_eq!(preview(&c, burn, None, Var::Damage), Decimal::from(2));
}

#[test]
fn multi_creature_targeting_previews_a_modifier_every_enemy_shares() {
    // Hook.ModifyDamage (Hook.cs:1495): with no target, an AllEnemies card in the hand takes the
    // value against each hittable enemy if they agree (int compare), else the untargeted one.
    let mut c = new_combat();
    let p = player(&c);
    let enemies = c.state.hittable_enemies();
    let card = give_card(&mut c, CardKind::Thunderclap, 0);
    let multi = |c: &Combat| {
        c.state
            .card_preview(card, None, PreviewMode::MultiCreatureTargeting)[0]
            .1
    };
    let first = enemies[0];
    with_engine(&mut c, |eng| {
        eng.apply_power(first, PowerKind::Vulnerable, Decimal::from(2), Some(p))
    });
    let shared = enemies.len() == 1;
    assert_eq!(multi(&c), Decimal::from(if shared { 6 } else { 4 }));
    for &e in &enemies[1..] {
        with_engine(&mut c, |eng| {
            eng.apply_power(e, PowerKind::Vulnerable, Decimal::from(2), Some(p))
        });
    }
    assert_eq!(multi(&c), Decimal::from(6));
    assert_eq!(preview(&c, card, None, Var::Damage), Decimal::from(4));
}

/// Every move of every monster an encounter can create has its declared intents.
#[test]
fn every_encounter_monster_move_has_declared_intents() {
    use crate::content::tables::declared_intents;
    let rng = RunRngSet::new(SEED);
    for encounter in Encounter::ALL {
        for floor in 0..8 {
            for (monster, _) in encounter.generate_monsters(&rng, floor) {
                for state in monster.model().moves().states {
                    if state.is_move() {
                        assert!(
                            declared_intents(monster.id(), state.id()).is_some(),
                            "{} {}",
                            monster.id(),
                            state.id()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn intents_pair_attacks_with_their_damage() {
    // Each attack intent gets its attack's damage; a stunned monster shows `StunIntent`
    // (Creature.cs:538).
    let mut c = Combat::new(
        CombatConfig::necrobinder(SEED, 0).with_encounter(Encounter::WaterfallGiantBoss),
    );
    let giant = enemy(&c);
    let intents = c.state.intents(giant);
    assert_ne!(intents, []);
    for intent in &intents {
        assert_eq!(
            intent.attack.is_some(),
            matches!(intent.kind, IntentType::Attack | IntentType::DeathBlow)
        );
    }
    with_engine(&mut c, |eng| {
        crate::content::monsters::stun(eng, giant, None);
        Ok(())
    });
    assert_eq!(
        c.state.intents(giant),
        vec![Intent {
            kind: IntentType::Stun,
            attack: None,
            cards: None,
            card: None,
            applies: Vec::new(),
        }]
    );
}
