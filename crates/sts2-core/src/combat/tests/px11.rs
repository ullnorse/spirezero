//! Engine gaps XT3's wider traces found (PX11).

use super::*;
use crate::content::powers::{PowerData, hellraiser};

fn facing(encounter: Encounter) -> Combat {
    Combat::new(CombatConfig::necrobinder(SEED, 0).with_encounter(encounter))
}

#[test]
fn a_waterfall_giant_explode_is_a_death_blow() {
    // WaterfallGiant.cs:205: EXPLODE_MOVE has a DeathBlowIntent, its other attacks a
    // SingleAttackIntent (RL2S5 underdocks step 262).
    let mut c = facing(Encounter::WaterfallGiantBoss);
    let giant = enemy(&c);
    let kinds = |c: &Combat| -> Vec<IntentType> {
        c.state
            .intent_damage(giant)
            .iter()
            .map(|a| a.kind)
            .collect()
    };
    let kind = c
        .state
        .creature(giant)
        .monster
        .as_ref()
        .expect("a monster")
        .kind;
    let states = kind.model().moves().states;
    let state_of = |id: &str| states.iter().position(|s| s.id() == id).expect("a state");
    for (id, kind) in [
        ("STOMP_MOVE", IntentType::Attack),
        ("EXPLODE_MOVE", IntentType::DeathBlow),
    ] {
        let monster = c
            .state
            .creature_mut(giant)
            .monster
            .as_mut()
            .expect("a monster");
        monster.machine.current = state_of(id);
        assert_eq!(kinds(&c), [kind], "{id}");
    }
}

#[test]
fn misery_copies_a_debuff_with_its_turn_start_amount() {
    // Misery.cs: each debuff is a ClonePreservingMutability (a MemberwiseClone,
    // AbstractModel.cs:173) applied to the other enemies, so a new instance keeps the source's
    // AmountOnTurnStart (pd7-genetic_algorithm, RL2S6 step 142: Doom onto the Wrigglers).
    let mut c = facing(Encounter::SlimesWeak);
    let (p, target) = (player(&c), enemy(&c));
    let other = c.state.enemies[1];
    with_engine(&mut c, |eng| {
        eng.apply_power(target, PowerKind::Doom, Decimal::from(5), Some(p))
    });
    let doom = |c: &Combat, e: CreatureId| -> Option<(i32, i32)> {
        c.state
            .creature(e)
            .powers
            .iter()
            .find(|p| p.kind == PowerKind::Doom)
            .map(|p| (p.amount, p.amount_on_turn_start))
    };
    c.state
        .creature_mut(target)
        .powers
        .iter_mut()
        .for_each(|p| p.amount_on_turn_start = 3);
    c.state.creature_mut(target).hp = 100;
    let misery = give_card(&mut c, CardKind::Misery, 0);
    c.apply(Action::PlayCard {
        card: misery,
        target: Some(target),
    })
    .expect("legal");
    assert_eq!(doom(&c, other), Some((5, 3)));
}

#[test]
fn misery_adds_a_temporary_debuff_to_its_stat() {
    // Misery.cs: an ITemporaryPower's amount is added to its internally applied power's entry
    // (CrushUnder 1 on top of Strength -3 copies Strength -2), then the CrushUnder copy takes
    // its own Strength (c9-act2 RL2S6 step 355: the Tough Eggs end at -4, not -5).
    let mut c = facing(Encounter::SlimesWeak);
    let (p, target) = (player(&c), enemy(&c));
    let other = c.state.enemies[1];
    with_engine(&mut c, |eng| {
        eng.apply_power(target, PowerKind::Strength, Decimal::from(-2), Some(p))?;
        eng.apply_power(target, PowerKind::CrushUnder, Decimal::ONE, Some(p))
    });
    let amount = |c: &Combat, e: CreatureId, kind: PowerKind| {
        c.state
            .creature(e)
            .powers
            .iter()
            .find(|p| p.kind == kind)
            .map(|p| p.amount)
    };
    assert_eq!(amount(&c, target, PowerKind::Strength), Some(-3));
    c.state.creature_mut(target).hp = 100;
    let misery = give_card(&mut c, CardKind::Misery, 0);
    c.apply(Action::PlayCard {
        card: misery,
        target: Some(target),
    })
    .expect("legal");
    assert_eq!(amount(&c, other, PowerKind::CrushUnder), Some(1));
    assert_eq!(amount(&c, other, PowerKind::Strength), Some(-3));
}

#[test]
fn hellraiser_auto_plays_at_most_nine_strikes_while_every_enemy_is_infinite() {
    // HellraiserPower.cs: with every hittable enemy's HpDisplay infinite (WaterfallGiant.cs:307,
    // HardenedShellPower.cs:66) the 10th Strike drawn in a turn stays in the hand; the owner's
    // turn end resets the count.
    let mut c = facing(Encounter::SlimesWeak);
    let p = player(&c);
    c.state.player.draw.clear();
    for e in c.state.enemies.clone() {
        let enemy = c.state.creature_mut(e);
        enemy.hp = 999;
        enemy.infinite_hp_display = true;
    }
    let strikes = with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Hellraiser, Decimal::ONE, Some(p))?;
        let strikes: Vec<CardId> = (0..10)
            .map(|_| eng.create_card(CardKind::StrikeNecrobinder))
            .collect();
        eng.add_generated_cards_to_combat(&strikes, PileType::Draw, None, Position::Bottom)?;
        Ok(strikes)
    });
    c.state.player.hand.clear();
    for _ in 0..10 {
        with_engine(&mut c, |eng| eng.draw(Decimal::ONE, false));
    }
    assert_eq!(*c.state.player.hand, [strikes[9]]);
    let hellraiser = power_data(&c, p);
    assert_eq!(
        hellraiser,
        PowerData::Hellraiser(hellraiser::Data {
            auto_playing_cards: 0,
            infinite_auto_plays_this_turn: 10,
            showed_cap_reached_message: true,
        })
    );
    let allies = c.state.allies.clone();
    with_engine(&mut c, |eng| eng.after_side_turn_end(Side::Player, &allies));
    assert_eq!(
        power_data(&c, p),
        PowerData::Hellraiser(hellraiser::Data::default())
    );
}

#[test]
fn pillage_returns_when_every_drawn_strike_leaves_the_hand() {
    // Pillage.cs loops while the drawn card is an Attack and the hand is not full. With Hellraiser
    // and Ringing (its play is vetoed once a card play started, so the Strike goes to the discard
    // pile unplayed) and only a Strike to draw, the game never leaves the loop; ours is bounded.
    let mut c = facing(Encounter::SlimesWeak);
    let (p, target) = (player(&c), enemy(&c));
    c.state.player.draw.clear();
    c.state.player.hand.clear();
    c.state.creature_mut(target).hp = 999;
    let pillage = give_card(&mut c, CardKind::Pillage, 0);
    let strike = with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Hellraiser, Decimal::ONE, Some(p))?;
        let strike = eng.create_card(CardKind::StrikeNecrobinder);
        eng.add_generated_cards_to_combat(&[strike], PileType::Discard, None, Position::Bottom)?;
        eng.apply_power(p, PowerKind::Ringing, Decimal::ONE, Some(p))?;
        Ok(strike)
    });
    c.state.player.energy = 3;
    let status = c
        .apply(Action::PlayCard {
            card: pillage,
            target: Some(target),
        })
        .expect("legal");
    assert_eq!(status, Status::PlayerTurn);
    assert!(c.state.creature(target).hp > 0);
    assert!(c.state.player.hand.is_empty());
    assert_eq!(c.state.pile_of(strike), Some(PileType::Discard));
}

fn power_data(c: &Combat, owner: CreatureId) -> PowerData {
    let id = c
        .state
        .creature(owner)
        .powers
        .iter()
        .find(|p| p.kind == PowerKind::Hellraiser)
        .expect("applied")
        .id;
    *c.state.power_data(id)
}
