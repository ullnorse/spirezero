//! Extra turns (`Ambergris`), `UnsettlingLamp`'s card source, `CrueltyPower` and `Alchemize`
//! (C14; paths under `ref/decomp/MegaCrit.Sts2.Core.`).

use super::*;
use crate::content::relics::OwnedRelic;
use crate::run::potions::random_potion_in_combat;

fn give_power(c: &mut Combat, creature: CreatureId, kind: PowerKind, amount: i32) {
    let applier = Some(player(c));
    with_engine(c, |eng| {
        eng.apply_power(creature, kind, Decimal::from(amount), applier)
    });
}

fn with_relic(kind: RelicKind) -> Combat {
    let mut config = CombatConfig::necrobinder(SEED, 0);
    config.relics.push(OwnedRelic {
        kind,
        data: kind.init_data(),
        melted: false,
    });
    Combat::new(config)
}

fn play(c: &mut Combat, kind: CardKind, target: Option<CreatureId>) -> CardId {
    let card = give_card(c, kind, 0);
    c.apply(Action::PlayCard { card, target }).expect("legal");
    card
}

#[test]
fn an_ambergris_stack_is_an_extra_player_turn() {
    // CombatManager.cs:1825-1895: `SwitchFromPlayerToEnemySide` asks `ShouldTakeExtraTurn`,
    // `SwitchSides` keeps the player side and adds a player turn but not a round, and
    // `AfterTakingExtraTurn` decrements the power. The extra `StartTurn` (`:699-758`) skips the
    // enemies' `PrepareForNextTurn`.
    let mut c = new_combat();
    let p = player(&c);
    let target = enemy(&c);
    give_power(&mut c, p, PowerKind::Ambergris, 2);
    let hp = c.state.creature(p).hp;
    let move_before = c
        .state
        .creature(target)
        .monster
        .as_ref()
        .map(Monster::next_move_name);
    c.apply(Action::EndTurn).expect("legal");
    let s = c.state();
    assert_eq!(
        (s.side, s.round, s.player.turn_number),
        (Side::Player, 1, 2)
    );
    assert_eq!(s.creature(p).hp, hp, "no enemy turn in between");
    assert_eq!(s.creature(p).power_amount(PowerKind::Ambergris), 1);
    assert_eq!(s.player.hand.len(), 5);
    assert_eq!(s.player.energy, 3);
    assert_eq!(
        s.creature(target)
            .monster
            .as_ref()
            .map(Monster::next_move_name),
        move_before,
        "the enemy keeps its move"
    );

    c.apply(Action::EndTurn).expect("legal");
    let s = c.state();
    assert_eq!(
        (s.side, s.round, s.player.turn_number),
        (Side::Player, 1, 3)
    );
    assert!(!s.creature(p).has_power(PowerKind::Ambergris), "used up");

    // The third end of turn is an ordinary one: the enemies act and the round advances.
    c.apply(Action::EndTurn).expect("legal");
    let s = c.state();
    assert_eq!(
        (s.side, s.round, s.player.turn_number),
        (Side::Player, 2, 4)
    );
    assert!(s.creature(p).hp < hp, "the enemy attacked");
}

#[test]
fn the_ambergris_potion_heals_and_grants_the_turn() {
    // Ambergris.cs: heal `MaxHp * 50 / 100` (not above max), then 1 `AmbergrisPower` while the
    // combat is in progress.
    let mut c = new_combat();
    let p = player(&c);
    c.state.creature_mut(p).hp = 10;
    let max_hp = c.state.creature(p).max_hp;
    c.edit(|s| {
        s.player.potions[0] = Some(Potion {
            id: PotionId(0),
            kind: PotionKind::Ambergris,
        });
    });
    c.apply(Action::UsePotion {
        slot: 0,
        target: Some(p),
    })
    .expect("legal");
    assert_eq!(c.state.creature(p).hp, 10 + max_hp * 50 / 100);
    assert_eq!(c.state.creature(p).power_amount(PowerKind::Ambergris), 1);
    assert!(!PowerKind::Ambergris.is_visible());
}

#[test]
fn the_lamp_doubles_the_first_cards_debuffs_only() {
    // UnsettlingLamp.cs: the first visible debuff a card applies to an enemy, and the other
    // debuffs of that card, are doubled; the lamp is then finished for the combat.
    let mut c = with_relic(RelicKind::UnsettlingLamp);
    let target = enemy(&c);
    play(&mut c, CardKind::Defy, Some(target));
    assert_eq!(c.state.creature(target).power_amount(PowerKind::Weak), 2);
    play(&mut c, CardKind::Defy, Some(target));
    assert_eq!(
        c.state.creature(target).power_amount(PowerKind::Weak),
        3,
        "the second card is plain"
    );
}

#[test]
fn the_lamp_does_not_double_a_temporary_powers_stat_again() {
    // UnsettlingLamp.HasDoubledTemporaryPowerSource: EnfeeblingTouch's power is doubled (16),
    // the Strength it removes is not doubled again (-16, not -32).
    let mut c = with_relic(RelicKind::UnsettlingLamp);
    let target = enemy(&c);
    let strength = c.state.creature(target).power_amount(PowerKind::Strength);
    play(&mut c, CardKind::EnfeeblingTouch, Some(target));
    let t = c.state.creature(target);
    assert_eq!(t.power_amount(PowerKind::EnfeeblingTouch), 16);
    assert_eq!(t.power_amount(PowerKind::Strength), strength - 16);
}

#[test]
fn the_lamp_ignores_debuffs_without_a_card_or_on_the_player() {
    // `cardSource == null` (a potion) and `target.Side == Owner.Creature.Side` do not trigger.
    let mut c = with_relic(RelicKind::UnsettlingLamp);
    let (p, target) = (player(&c), enemy(&c));
    give_power(&mut c, target, PowerKind::Weak, 1);
    give_power(&mut c, p, PowerKind::Weak, 1);
    assert_eq!(c.state.creature(target).power_amount(PowerKind::Weak), 1);
    assert_eq!(c.state.creature(p).power_amount(PowerKind::Weak), 1);
    play(&mut c, CardKind::Defy, Some(target));
    assert_eq!(
        c.state.creature(target).power_amount(PowerKind::Weak),
        3,
        "the card still triggers: 1 + 2"
    );
}

#[test]
fn cruelty_strengthens_vulnerable_for_its_owner() {
    // CrueltyPower.cs / VulnerablePower.cs: the multiplier becomes 1.5 + Amount / 100 for the
    // owner's powered attacks.
    let mut base = new_combat();
    let mut cruel = new_combat();
    for c in [&mut base, &mut cruel] {
        let target = enemy(c);
        give_power(c, target, PowerKind::Vulnerable, 2);
    }
    let p = player(&cruel);
    give_power(&mut cruel, p, PowerKind::Cruelty, 50);
    let mut hits = [0; 2];
    for (c, hit) in [&mut base, &mut cruel].into_iter().zip(&mut hits) {
        let target = enemy(c);
        let hp = c.state.creature(target).hp;
        play(c, CardKind::StrikeNecrobinder, Some(target));
        *hit = hp - c.state.creature(target).hp;
    }
    // 6 x 1.5 = 9 and 6 x 2.0 = 12.
    assert_eq!(hits, [9, 12]);
}

#[test]
fn alchemize_procures_the_pools_next_combat_potion() {
    // Alchemize.cs, PotionFactory.cs:64-74: one pick on `CombatPotionGeneration`, from the
    // options that can be generated in combat.
    let mut c = new_combat();
    let mut rng = c.state.rng[RunRngType::CombatPotionGeneration].clone();
    let id = random_potion_in_combat(Character::Necrobinder, &mut rng);
    play(&mut c, CardKind::Alchemize, None);
    let potion = c.state.player.potions.iter().flatten().next();
    assert_eq!(
        potion.map(|p| crate::run::potions::potion_id(p.kind)),
        Some(id)
    );
    assert_eq!(
        c.state.rng[RunRngType::CombatPotionGeneration].counter(),
        rng.counter()
    );
}
