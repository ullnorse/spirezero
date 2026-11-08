//! Engine-internal tests: hand-built situations that need direct state access. Each test cites
//! the decomp behaviour it checks (paths under `ref/decomp/MegaCrit.Sts2.Core.`).

use super::*;
use crate::content::cards::{CardRarity, CardType, pools};
use crate::content::characters::Character;
use crate::content::potions::PotionKind;
use crate::content::powers::PowerKind;
use crate::content::relics::RelicKind;
use crate::decimal::Decimal;
use crate::rng::RunRngType;
use commands::Position;
use hooks::HookSet;
use listeners::Entry;
use state::MovedCards;
use state::Power;

const SEED: &str = "94LYYA8YDC";

fn new_combat() -> Combat {
    Combat::new(CombatConfig::necrobinder(SEED, 0).with_events())
}

fn player(c: &Combat) -> CreatureId {
    c.state.player.creature
}

fn enemy(c: &Combat) -> CreatureId {
    c.state.enemies[0]
}

fn osty(c: &Combat) -> CreatureId {
    c.state.osty().expect("Bound Phylactery summoned Osty")
}

/// Puts a fresh card of `kind` into the hand and returns it.
fn give_card(c: &mut Combat, kind: CardKind, upgrade_level: u8) -> CardId {
    c.edit(|s| {
        let id = CardId(u32::try_from(s.cards.len()).expect("fits"));
        s.cards.push(Card::new(id, kind, upgrade_level));
        s.insert_into_pile(id, PileType::Hand, None);
        id
    })
}

/// Runs `f` inside an engine with no recorded decisions.
fn with_engine<T>(c: &mut Combat, f: impl FnOnce(&mut Engine<'_>) -> Flow<T>) -> T {
    let mut eng = Engine::new(&mut c.state, c.events.as_mut(), &[]);
    let result = f(&mut eng).expect("no decision needed");
    c.resync();
    result
}

#[test]
fn block_removal_truncates_decimal_damage() {
    // Creature.cs:431-466, CreatureCmd.cs:276-311: 5.25 into 10 block removes 5 block and
    // the .25 is lost; 5.25 into 3 block leaks 2.25 and loses 2 HP.
    let mut c = new_combat();
    let p = player(&c);
    c.state.creature_mut(p).block = 10;
    // Unpowered so Osty does not redirect.
    let props = ValueProps::UNPOWERED;
    with_engine(&mut c, |eng| {
        eng.damage(&[p], Decimal::new(525, 2), props, None, None)
    });
    assert_eq!(c.state.creature(p).block, 5);
    assert_eq!(c.state.creature(p).hp, 66);

    c.state.creature_mut(p).block = 3;
    with_engine(&mut c, |eng| {
        eng.damage(&[p], Decimal::new(525, 2), props, None, None)
    });
    assert_eq!(c.state.creature(p).block, 0);
    assert_eq!(c.state.creature(p).hp, 64);
}

#[test]
fn hitting_osty_spends_the_owners_block() {
    // CreatureCmd.cs:285: blockHolder = target.PetOwner?.Creature ?? target.
    let mut c = new_combat();
    let (p, o) = (player(&c), osty(&c));
    c.state.creature_mut(p).block = 3;
    let props = ValueProps::MOVE;
    with_engine(&mut c, |eng| {
        eng.damage(&[o], Decimal::from(2), props, None, None)
    });
    assert_eq!(c.state.creature(p).block, 1);
    assert_eq!(c.state.creature(o).hp, 1, "fully blocked");
}

#[test]
fn osty_redirect_overkill_and_survival_after_death() {
    // DieForYouPower.cs:15-52, CreatureCmd.cs:289-311: unblocked powered damage aimed at the
    // owner goes to Osty; the overkill comes back to the owner; dead Osty stays in combat.
    let mut c = new_combat();
    let (p, o, e) = (player(&c), osty(&c), enemy(&c));
    c.state.creature_mut(o).max_hp = 3;
    c.state.creature_mut(o).hp = 3;
    c.state.creature_mut(p).block = 2;
    with_engine(&mut c, |eng| {
        eng.damage(&[p], Decimal::from(7), ValueProps::MOVE, Some(e), None)
    });
    assert_eq!(c.state.creature(p).block, 0);
    assert_eq!(c.state.creature(o).hp, 0);
    assert_eq!(c.state.creature(p).hp, 66 - 2, "7 - 2 block - 3 Osty HP");
    assert!(c.state.allies.contains(&o) && c.state.osty() == Some(o));
    assert_eq!(c.state.creature(o).power_amount(PowerKind::DieForYou), 1);

    // With Osty dead nothing is redirected.
    with_engine(&mut c, |eng| {
        eng.damage(&[p], Decimal::from(4), ValueProps::MOVE, Some(e), None)
    });
    assert_eq!(c.state.creature(p).hp, 60);

    // Unpowered damage is never redirected (DieForYouPower.cs:25).
    let mut c = new_combat();
    let (p, o) = (player(&c), osty(&c));
    with_engine(&mut c, |eng| {
        eng.damage(&[p], Decimal::from(4), ValueProps::UNPOWERED, None, None)
    });
    assert_eq!(c.state.creature(o).hp, 1);
    assert_eq!(c.state.creature(p).hp, 62);
}

#[test]
fn summon_grows_living_osty_and_revives_dead_osty_at_the_amount() {
    // OstyCmd.cs:48-78: alive -> +max and +current; dead -> max HP reset to the amount.
    let mut c = new_combat();
    let o = osty(&c);
    with_engine(&mut c, |eng| eng.summon_osty(Decimal::from(5)));
    assert_eq!((c.state.creature(o).hp, c.state.creature(o).max_hp), (6, 6));
    c.state.creature_mut(o).hp = 0;
    with_engine(&mut c, |eng| eng.summon_osty(Decimal::from(5)));
    assert_eq!((c.state.creature(o).hp, c.state.creature(o).max_hp), (5, 5));
    assert_eq!(c.state.osty(), Some(o), "revived, not re-created");
}

#[test]
fn unleash_scales_with_osty_hp_and_ignores_player_strength() {
    // Unleash.cs:20-49, CalculatedVar.cs:62-70, StrengthPower.cs:16: 6 (+3 upgraded) plus
    // Osty's current HP, dealt by Osty, so the player's Strength does not apply.
    for (upgrade, base) in [(0, 6), (1, 9)] {
        let mut c = new_combat();
        let (p, o, e) = (player(&c), osty(&c), enemy(&c));
        c.state.creature_mut(o).max_hp = 4;
        c.state.creature_mut(o).hp = 4;
        with_engine(&mut c, |eng| {
            eng.apply_power(p, PowerKind::Strength, Decimal::from(3), Some(p))
        });
        let card = give_card(&mut c, CardKind::Unleash, upgrade);
        let hp = c.state.creature(e).hp;
        c.apply(Action::PlayCard {
            card,
            target: Some(e),
        })
        .expect("legal");
        assert_eq!(c.state.creature(e).hp, hp - (base + 4));
        assert_eq!(c.state.pile_of(card), Some(PileType::Discard));
    }
}

#[test]
fn unleash_without_osty_does_nothing_but_still_costs_energy() {
    // Unleash.cs:39: Osty.CheckMissingWithAnim skips the attack only.
    let mut c = new_combat();
    let (o, e) = (osty(&c), enemy(&c));
    c.state.creature_mut(o).hp = 0;
    let card = give_card(&mut c, CardKind::Unleash, 0);
    let (hp, energy) = (c.state.creature(e).hp, c.state.player.energy);
    c.apply(Action::PlayCard {
        card,
        target: Some(e),
    })
    .expect("legal");
    assert_eq!(c.state.creature(e).hp, hp);
    assert_eq!(c.state.player.energy, energy - 1);
    assert_eq!(c.state.pile_of(card), Some(PileType::Discard));
}

#[test]
fn player_strength_applies_to_strike() {
    // StrengthPower.cs:16: +Amount when the owner deals a powered attack.
    let mut c = new_combat();
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Strength, Decimal::from(2), Some(p))
    });
    let card = give_card(&mut c, CardKind::StrikeNecrobinder, 1);
    let hp = c.state.creature(e).hp;
    c.apply(Action::PlayCard {
        card,
        target: Some(e),
    })
    .expect("legal");
    assert_eq!(c.state.creature(e).hp, hp - 11);
}

#[test]
fn lethal_card_wins_at_the_executor_safe_point() {
    // ActionExecutor.cs:170, CombatManager.cs:1297-1330, CardPileCmd.cs:381: the win is
    // processed after the action; the dead enemy is removed from combat; adding to a combat
    // pile is a no-op once combat is ending (so the lethal card stays in the Play pile until
    // the teardown); the player's teardown then clears the piles, powers and block.
    let mut c = new_combat();
    let e = enemy(&c);
    c.state.creature_mut(e).hp = 6;
    let card = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let mut ending = c.clone();
    with_engine(&mut ending, |eng| {
        eng.damage(&[e], Decimal::from(6), ValueProps::NONE, None, None)?;
        assert!(eng.s.is_ending());
        eng.add_to_pile(card, PileType::Discard, Position::Bottom)
    });
    assert_eq!(ending.state.pile_of(card), Some(PileType::Hand));

    let p = player(&c);
    c.state.creature_mut(p).block = 3;
    let status = c
        .apply(Action::PlayCard {
            card,
            target: Some(e),
        })
        .expect("legal");
    assert_eq!(status, Status::Won);
    assert_eq!(c.state.enemies, []);
    assert_eq!(c.state.pile_of(card), None);
    assert!(
        PileType::LISTENER_ORDER
            .iter()
            .all(|&pile| c.state.player.pile(pile).is_empty())
    );
    assert_eq!(c.state.creature(p).block, 0);
    assert_eq!(c.legal_actions(), []);
    assert_eq!(c.apply(Action::EndTurn), Err(ActionError::CombatOver));
}

#[test]
fn illegal_plays_are_rejected_without_changes() {
    let mut c = new_combat();
    let e = enemy(&c);
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let defend = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    let before = c.clone();
    let play = |card, target| Action::PlayCard { card, target };
    assert_eq!(c.apply(play(strike, None)), Err(ActionError::InvalidTarget));
    assert_eq!(
        c.apply(play(defend, Some(e))),
        Err(ActionError::InvalidTarget)
    );
    assert_eq!(
        c.apply(play(CardId(999), None)),
        Err(ActionError::CardNotInHand)
    );
    c.state.player.energy = 0;
    assert_eq!(
        c.apply(play(defend, None)),
        Err(ActionError::NotEnoughEnergy)
    );
    c.state.player.energy = before.state.player.energy;
    assert_eq!(c, before);
}

#[test]
fn mid_effect_choice_aborts_and_replays_with_the_decision_log() {
    // Execution model C (docs/decisions.md): two choices inside one card play.
    let mut c = new_combat();
    let card = give_card(&mut c, CardKind::TestChoice, 0);
    let hand: Vec<CardId> = c.state.player.hand.to_vec();
    let (first, second) = (hand[0], hand[1]);

    assert_eq!(
        c.apply(Action::PlayCard { card, target: None }),
        Ok(Status::Decision)
    );
    // The state shown is the state at the choice: the card is in the Play pile.
    assert_eq!(c.state.pile_of(card), Some(PileType::Play));
    let Some(DecisionRequest::ChooseCards {
        options,
        min: 1,
        max: 1,
        picked,
        prompt,
    }) = c.pending_decision()
    else {
        panic!("expected a hand choice");
    };
    assert_eq!(options.len(), 5);
    assert_eq!(picked, &[]);
    // The card asks, with `CardSelectorPrefs.DiscardSelectionPrompt`.
    assert_eq!(*prompt, Prompt::card(card, PromptText::Discard));
    assert_eq!(
        prompt.key(&c.state),
        Some(("card_selection", "TO_DISCARD".to_owned()))
    );
    // One pick per option; no `Done` before the minimum is picked.
    assert_eq!(c.legal_actions().len(), 5);
    assert_eq!(
        c.apply(Action::Resolve(Decision::Done)),
        Err(ActionError::InvalidDecision)
    );
    assert_eq!(c.apply(Action::EndTurn), Err(ActionError::DecisionPending));
    assert_eq!(
        c.apply(Action::Resolve(Decision::Cards(vec![first, second]))),
        Err(ActionError::InvalidDecision)
    );

    assert_eq!(
        c.apply(Action::Resolve(Decision::Cards(vec![first]))),
        Ok(Status::Decision)
    );
    assert_eq!(c.state.pile_of(first), Some(PileType::Discard));
    // A pick that reaches `max` completes the choice.
    assert_eq!(
        c.apply(Action::Resolve(Decision::Pick(second))),
        Ok(Status::PlayerTurn)
    );
    assert_eq!(c.state.pile_of(card), Some(PileType::Discard));
    assert_eq!(&c.state.player.discard[..2], &[first, second]);
    // 5 + test card - test card - 2 discarded + 1 drawn.
    assert_eq!(c.state.player.hand.len(), 4);

    // Replaying from scratch with the full log gives the same state and events.
    let mut direct = new_combat();
    let card = give_card(&mut direct, CardKind::TestChoice, 0);
    let snapshot = direct.state.clone();
    let decisions = [Decision::Cards(vec![first]), Decision::Cards(vec![second])];
    let mut eng = Engine::new(&mut direct.state, direct.events.as_mut(), &decisions);
    eng.play_card_action(card, None)
        .expect("all decisions supplied");
    assert_ne!(direct.state, snapshot);
    assert_eq!(direct.state, c.state);
    assert_eq!(direct.events, c.events);
}

#[test]
fn card_choices_are_sequential_picks() {
    let request = DecisionRequest::ChooseCards {
        options: vec![CardId(0), CardId(1), CardId(2)],
        min: 1,
        max: 2,
        picked: vec![CardId(1)],
        prompt: Prompt::card(CardId(0), PromptText::Own),
    };
    assert_eq!(
        request.options(),
        [
            Decision::Pick(CardId(0)),
            Decision::Pick(CardId(2)),
            Decision::Done
        ]
    );
    assert!(!request.accepts(&Decision::Pick(CardId(1))));
    assert!(!request.accepts(&Decision::Cards(vec![CardId(0)])));
    assert!(!request.is_complete());
    let mut full = request;
    full.pick(CardId(2));
    assert!(full.is_complete());
    assert_eq!(full.options(), [Decision::Done]);
}

#[test]
fn snapshots_are_taken_only_while_a_model_may_prompt() {
    let mut c = new_combat();
    assert!(!c.state.may_prompt());
    give_card(&mut c, CardKind::TestChoice, 0);
    assert!(c.state.may_prompt());
}

#[test]
fn choice_is_skipped_when_it_leaves_no_freedom() {
    // CardSelectCmd.cs:479-483: no options -> empty; exact count >= options -> all.
    let mut c = new_combat();
    let hand = c.state.player.hand.to_vec();
    let prompt = Prompt::card(hand[0], PromptText::Own);
    let picked = with_engine(&mut c, |eng| {
        eng.choose_cards(prompt, hand[..1].to_vec(), 1, 1)
    });
    assert_eq!(picked, hand[..1]);
    let picked = with_engine(&mut c, |eng| eng.choose_cards(prompt, Vec::new(), 1, 1));
    assert_eq!(picked, []);
}

/// The potion kind in each belt slot.
fn belt(c: &Combat) -> Vec<Option<PotionKind>> {
    c.state
        .player
        .potions
        .iter()
        .map(|p| p.map(|p| p.kind))
        .collect()
}

fn with_potions(potions: &[PotionKind]) -> Combat {
    let mut config = CombatConfig::necrobinder(SEED, 0);
    config.potions = potions.iter().map(|&k| Some(k.into())).collect();
    Combat::new(config)
}

#[test]
fn potions_fill_slots_in_order() {
    // Player.cs:28 (3 slots), AscensionManager.cs:56 (TightBelt takes one).
    let c = with_potions(&[PotionKind::FirePotion]);
    assert_eq!(belt(&c), [Some(PotionKind::FirePotion), None, None]);
    assert_eq!(CombatConfig::necrobinder(SEED, 4).potion_slots, 2);
}

#[test]
fn fire_potion_empties_its_slot_and_deals_unpowered_damage() {
    // PotionModel.cs:297 (RemoveBeforeUse first), FirePotion.cs: 20 unpowered damage, so the
    // player's Strength does not add to it.
    let mut c = with_potions(&[PotionKind::FirePotion, PotionKind::BlockPotion]);
    let (p, e) = (player(&c), enemy(&c));
    c.edit(|s| {
        s.creature_mut(p).push_power(Power {
            id: PowerId(99),
            kind: PowerKind::Strength,
            amount: 5,
            amount_on_turn_start: 5,
            applier: Some(p),
            skip_next_duration_tick: false,
        });
    });
    let hp = c.state.creature(e).hp;
    let use_fire = Action::UsePotion {
        slot: 0,
        target: Some(e),
    };
    assert!(c.legal_actions().contains(&use_fire));
    c.apply(use_fire.clone()).expect("legal");
    assert_eq!(c.state.creature(e).hp, (hp - 20).max(0));
    assert_eq!(belt(&c), [None, Some(PotionKind::BlockPotion), None]);
    assert_eq!(c.apply(use_fire), Err(ActionError::NoPotionInSlot));
}

#[test]
fn player_potions_target_the_player_by_default() {
    // PotionModel.EnqueueManualUse (cs:246): no target means the owner. BlockPotion.cs: 12
    // block; EnergyPotion.cs: 2 energy (PlayerCmd.cs:29).
    let mut c = with_potions(&[PotionKind::BlockPotion, PotionKind::EnergyPotion]);
    let p = player(&c);
    let (block, energy) = (c.state.creature(p).block, c.state.player.energy);
    c.apply(Action::UsePotion {
        slot: 0,
        target: None,
    })
    .expect("legal");
    assert_eq!(c.state.creature(p).block, block + 12);
    c.apply(Action::UsePotion {
        slot: 1,
        target: Some(p),
    })
    .expect("legal");
    assert_eq!(c.state.player.energy, energy + 2);
    let enemy_target = Action::UsePotion {
        slot: 0,
        target: Some(enemy(&c)),
    };
    assert!(c.state.player.potions.iter().all(Option::is_none));
    assert_eq!(c.apply(enemy_target), Err(ActionError::NoPotionInSlot));
}

#[test]
fn block_potion_cannot_target_an_enemy() {
    // PotionModel.IsValidTarget (cs:286): AnyPlayer needs a player.
    let mut c = with_potions(&[PotionKind::BlockPotion]);
    let action = Action::UsePotion {
        slot: 0,
        target: Some(enemy(&c)),
    };
    assert!(!c.legal_actions().contains(&action));
    assert_eq!(c.apply(action), Err(ActionError::InvalidTarget));
}

#[test]
fn intent_damage_runs_the_damage_hooks() {
    // AttackIntent.cs:107: ModifyDamage with the player as target and ValueProp.Move. The
    // Fuzzy Wurm Crawler opens with Acid Goop, 4 damage.
    let mut c = new_combat();
    let e = enemy(&c);
    let before = c.state.clone();
    assert_eq!(
        c.state.intent_damage(e),
        [AttackIntent {
            kind: IntentType::Attack,
            damage: 4,
            hits: 1
        }]
    );
    assert_eq!(c.state, before, "the state is untouched");
    with_engine(&mut c, |eng| {
        eng.apply_power(e, PowerKind::Strength, Decimal::from(3), Some(e))
    });
    assert_eq!(
        c.state.intent_damage(e),
        [AttackIntent {
            kind: IntentType::Attack,
            damage: 7,
            hits: 1
        }]
    );
}

/// Sets up a combat without starting it, lets `prepare` edit the state, then starts it.
fn start_with(prepare: impl FnOnce(&mut CombatState)) -> Combat {
    let mut state = set_up(CombatConfig::necrobinder(SEED, 0));
    prepare(&mut state);
    state.recount_hooks();
    let mut c = Combat {
        state,
        events: Some(Vec::new()),
        pending: None,
        spare: None,
        snapshot: None,
        unsupported: None,
    };
    c.run(Step::StartCombat, Vec::new(), false);
    c
}

#[test]
fn innate_cards_start_on_top_of_the_draw_pile_last_first() {
    // CombatManager.cs:906-919: each Innate card in pile order goes to the top
    // (MoveToTopInternal), and the hand draw is at least the Innate count.
    let mut draw = Vec::new();
    let c = start_with(|s| {
        draw.clone_from(&s.player.draw);
        s.card_mut(draw[7]).keywords.insert(CardKeyword::Innate);
        s.card_mut(draw[9]).keywords.insert(CardKeyword::Innate);
    });
    assert_eq!(
        *c.state.player.hand,
        [draw[9], draw[7], draw[0], draw[1], draw[2]]
    );

    let c = start_with(|s| {
        for &card in &s.player.draw.to_vec()[..6] {
            s.card_mut(card).keywords.insert(CardKeyword::Innate);
        }
    });
    assert_eq!(c.state.player.hand.len(), 6);
}

#[test]
fn an_upgrade_caps_absolute_cost_modifiers_and_a_clone_has_its_own() {
    // CardEnergyCost.UpgradeBy: a lower base caps every absolute local modifier at it;
    // CardModel.CreateClone copies the energy cost with its modifiers.
    let mut c = new_combat();
    let slam = give_card(&mut c, CardKind::BodySlam, 0);
    c.state.energy_cost_mut(slam).set_this_combat(1);
    c.state.energy_cost_mut(slam).add_this_combat(1);
    assert_eq!(c.state.energy_cost(slam), 2);
    let clone = with_engine(&mut c, |eng| Ok(eng.create_clone(slam)));
    assert_eq!(c.state.energy_cost(clone), 2);
    with_engine(&mut c, |eng| {
        eng.upgrade_card(slam);
        Ok(())
    });
    assert_eq!(c.state.energy_cost(slam), 1, "set 1 capped at 0, then +1");
    assert_eq!(
        c.state.energy_cost(clone),
        2,
        "the clone's modifiers are its own"
    );
}

#[test]
fn local_cost_modifiers_fold_in_order_and_expire() {
    // CardEnergyCost.cs: modifiers apply in insertion order; EndOfTurnCleanup drops the
    // turn-scoped ones, AfterCardPlayedCleanup the until-played ones.
    let mut c = new_combat();
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    c.state.energy_cost_mut(strike).add_this_combat(1);
    assert_eq!(c.state.energy_cost(strike), 2);
    c.state.energy_cost_mut(strike).set_this_turn(0);
    assert_eq!(c.state.energy_cost(strike), 0);
    c.apply(Action::EndTurn).expect("legal");
    assert_eq!(c.state.energy_cost(strike), 2, "the this-combat +1 stays");

    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    c.state.energy_cost_mut(strike).set_until_played(0);
    let energy = c.state.player.energy;
    let target = Some(enemy(&c));
    c.apply(Action::PlayCard {
        card: strike,
        target,
    })
    .expect("legal");
    assert_eq!(c.state.player.energy, energy, "played for free");
    assert_eq!(c.state.energy_cost(strike), 1);
}

#[test]
fn single_turn_retain_keeps_a_card_for_one_flush() {
    // CombatManager.cs:1799-1818: ShouldRetainThisTurn cards stay in hand; EndOfTurnCleanup
    // then clears HasSingleTurnRetain.
    let mut c = new_combat();
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    c.state.card_mut(strike).single_turn_retain = true;
    c.apply(Action::EndTurn).expect("legal");
    assert!(c.state.player.hand.contains(&strike));
    assert!(!c.state.card(strike).single_turn_retain);
}

#[test]
fn upgrades_change_keywords_and_cost() {
    // Wisp.OnUpgrade adds Retain; Eidolon.OnUpgrade is EnergyCost.UpgradeBy(-1).
    let wisp = Card::new(CardId(0), CardKind::Wisp, 1);
    assert!(wisp.has_local(CardKeyword::Retain) && wisp.has_local(CardKeyword::Exhaust));
    assert!(!Card::new(CardId(0), CardKind::Wisp, 0).has_local(CardKeyword::Retain));
    assert_eq!(
        Card::new(CardId(0), CardKind::Eidolon, 1).energy_cost.base,
        1
    );
}

#[test]
fn x_cost_spends_all_energy_and_logs_it() {
    // CardModel.cs:1807-1834: GetAmountToSpend is all the energy, captured as X.
    let mut c = new_combat();
    let eradicate = give_card(&mut c, CardKind::Eradicate, 0);
    let target = Some(enemy(&c));
    c.apply(Action::PlayCard {
        card: eradicate,
        target,
    })
    .expect("legal");
    assert_eq!(c.state.player.energy, 0);
    assert_eq!(c.state.card(eradicate).energy_cost.captured_x, 3);
    assert!(
        c.state
            .history_this_turn(HistoryTag::EnergySpent)
            .iter()
            .any(|e| e.kind == HistoryKind::EnergySpent(3))
    );
}

#[test]
fn autoplay_moves_an_unplayable_card_to_its_result_pile() {
    // CardCmd.cs:51-66,133-137: Unplayable -> Play pile -> discard (not Exhaust-keyworded).
    let mut c = new_combat();
    let defile = give_card(&mut c, CardKind::Defile, 0);
    c.state.remove_from_pile(defile);
    c.state.insert_into_pile(defile, PileType::Exhaust, None);
    c.state
        .card_mut(defile)
        .keywords
        .insert(CardKeyword::Unplayable);
    let hp = c.state.creature(enemy(&c)).hp;
    with_engine(&mut c, |eng| eng.auto_play(defile, None));
    assert_eq!(c.state.pile_of(defile), Some(PileType::Discard));
    assert_eq!(c.state.creature(enemy(&c)).hp, hp);
}

#[test]
fn modify_keywords_in_combat_adds_global_keywords() {
    // CardModel.cs:1153-1181 (`Keywords` = local + Hook.ModifyKeywordsInCombat, Hook.cs:1603);
    // HexPower.cs (TryModifyKeywordsInCombat adds Ethereal to Hexed cards), via `TestHex`.
    let mut c = new_combat();
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let defend = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::TestHex, Decimal::ONE, None)?;
        eng.afflict(
            strike,
            crate::content::afflictions::AfflictionKind::TestDummy,
            Decimal::ONE,
        );
        Ok(())
    });
    assert!(c.state.has_keyword(strike, CardKeyword::Ethereal));
    assert!(
        !c.state.card(strike).has_local(CardKeyword::Ethereal),
        "global, never stored"
    );
    assert!(!c.state.has_keyword(defend, CardKeyword::Ethereal));
    // The global Ethereal counts at turn end (CombatManager.cs:1615): only the Strike exhausts.
    c.apply(Action::EndTurn).expect("legal");
    assert_eq!(c.state.pile_of(strike), Some(PileType::Exhaust));
    assert_ne!(c.state.pile_of(defend), Some(PileType::Exhaust));
}

mod c14;
mod c16;
mod c17;
mod c18;
mod c19;
mod c21;
mod c22;
mod c23;
mod c27;
mod c28;
mod content_gaps;
mod cq1;
mod engine_gaps;
mod fix1;
mod glow;
mod gm3;
mod pd3;
mod pd5;
mod pi5;
mod pu2;
mod px11;
mod px13;
mod px26;
mod px8;
mod xc6;
mod xev1;
mod xgm2;
mod xhm4;

#[test]
fn generating_an_unported_card_stops_the_combat_where_the_game_would() {
    // CardFactory.cs:119-129: the whole filtered pool is shuffled before the picks are created,
    // so the stop comes after the shuffle's draws.
    let mut c = new_combat();
    // Every single-player pool card is ported now, so the pool card is made up.
    let unported = [pools::PoolCard {
        id: "UNPORTED_CARD",
        rarity: CardRarity::Rare,
        card_type: CardType::Skill,
        ethereal: false,
        can_be_generated_in_combat: true,
    }];
    let counter = c.state.rng[RunRngType::CombatCardGeneration].counter();
    let mut eng = Engine::new(&mut c.state, None, &[]);
    assert_eq!(
        eng.get_distinct_for_combat(&unported, 1),
        Err(Box::new(Interrupt::Unsupported(Unsupported::card(
            "UNPORTED_CARD"
        ))))
    );
    // A one-card shuffle draws nothing.
    assert_eq!(
        c.state.rng[RunRngType::CombatCardGeneration].counter(),
        counter
    );
}

#[test]
fn instanced_powers_add_an_instance_per_application() {
    // PowerCmd.cs:168-178: `Instanced` finds no instance to stack onto; Creature.cs:616 appends.
    let mut c = new_combat();
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::TheBomb, Decimal::from(3), Some(p))?;
        eng.apply_power(p, PowerKind::Strength, Decimal::ONE, Some(p))?;
        eng.apply_power(p, PowerKind::TheBomb, Decimal::from(2), Some(p))?;
        eng.apply_power(p, PowerKind::Strength, Decimal::ONE, Some(p))
    });
    let powers: Vec<_> = c
        .state
        .creature(p)
        .powers
        .iter()
        .map(|p| (p.kind, p.amount))
        .collect();
    assert_eq!(
        powers,
        [
            (PowerKind::TheBomb, 3),
            (PowerKind::Strength, 2),
            (PowerKind::TheBomb, 2)
        ]
    );
}

#[test]
fn nostalgia_puts_the_first_attack_on_top_of_the_draw_pile() {
    // NostalgiaPower.cs: Attacks and Skills past `Amount` plays this turn are not redirected.
    let mut c = new_combat();
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Nostalgia, Decimal::ONE, Some(p))
    });
    let first = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let second = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    for card in [first, second] {
        c.apply(Action::PlayCard {
            card,
            target: Some(e),
        })
        .expect("legal");
    }
    assert_eq!(c.state.player.draw.first(), Some(&first));
    assert_eq!(c.state.pile_of(second), Some(PileType::Discard));
}

#[test]
fn rolling_boulder_instances_hit_at_turn_start_and_grow() {
    // RollingBoulderPower.cs (test-mode branch): each instance deals `Amount` unpowered damage
    // to every hittable enemy after the player's turn starts, then `SetAmount(Amount + 5)`.
    let mut c = new_combat();
    let (p, e) = (player(&c), enemy(&c));
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::RollingBoulder, Decimal::from(5), Some(p))?;
        eng.apply_power(p, PowerKind::RollingBoulder, Decimal::from(10), Some(p))
    });
    c.state.creature_mut(e).hp = 100;
    c.state.creature_mut(e).max_hp = 100;
    c.state.creature_mut(e).block = 0;
    let hp = c.state.creature(e).hp;
    with_engine(&mut c, |eng: &mut Engine<'_>| -> Flow {
        eng.after_player_turn_start()
    });
    assert_eq!(c.state.creature(e).hp, hp - 15);
    let amounts: Vec<_> = c
        .state
        .creature(p)
        .powers
        .iter()
        .map(|p| p.amount)
        .collect();
    assert_eq!(amounts, [10, 15]);
}
mod ra_relics;
mod relic_status;

#[test]
fn enthralled_vetoes_only_manual_plays() {
    // `Enthralled.ShouldPlay`: an `Enthralled` in hand vetoes `AutoPlayType.None` only, so an
    // auto-played Strike still deals its damage.
    let mut c = Combat::new(CombatConfig::necrobinder(SEED, 0));
    give_card(&mut c, CardKind::Enthralled, 0);
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let target = enemy(&c);
    assert!(
        c.apply(Action::PlayCard {
            card: strike,
            target: Some(target)
        })
        .is_err()
    );
    let hp = c.state.creature(target).hp;
    with_engine(&mut c, |eng| eng.auto_play(strike, Some(target)));
    assert_eq!(c.state.creature(target).hp, hp - 6);
}

#[test]
fn history_course_auto_plays_a_dupe_of_last_turns_attack() {
    // `HistoryCourse`: on turn 2 a dupe of turn 1's last Attack is auto-played; the dupe goes
    // nowhere afterwards, so the piles hold the same cards.
    let mut config = CombatConfig::necrobinder(SEED, 0);
    config.relics.push(RelicKind::HistoryCourse.into());
    let mut c = Combat::new(config);
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    let target = enemy(&c);
    c.apply(Action::PlayCard {
        card: strike,
        target: Some(target),
    })
    .expect("a Strike");
    let piled = |c: &Combat| {
        PileType::LISTENER_ORDER
            .iter()
            .map(|&p| c.state.player.pile(p).len())
            .sum::<usize>()
    };
    let before = piled(&c);
    let cards = c.state.cards.len();
    let hp = c.state.creature(target).hp;
    c.apply(Action::EndTurn).expect("end turn");
    assert_eq!(c.state.cards.len(), cards + 1, "one dupe");
    assert!(
        c.state
            .cards
            .last()
            .is_some_and(|d| d.is_dupe && d.pile.is_none())
    );
    assert!(c.state.creature(target).hp <= hp - 6);
    assert_eq!(piled(&c), before);
}

#[test]
fn the_listener_list_is_the_full_list_without_hookless_listeners() {
    // PF3: dropping listeners that override no hook keeps every hook's listeners and their
    // order, since every reader filters by hook.
    for character in Character::ALL {
        let config = CombatConfig::new(character, SEED, 0).expect("starter decks are ported");
        let mut c = Combat::new(config);
        for _ in 0..200 {
            let mut full = Vec::new();
            c.state
                .for_each_listener(|listener, hooks| full.push(Entry { listener, hooks }));
            full.retain(|e| !e.hooks.is_empty());
            let union = full.iter().fold(HookSet::EMPTY, |u, e| u.union(e.hooks));
            let mut sparse = Vec::new();
            assert_eq!(c.state.listener_entries(&mut sparse), union);
            assert_eq!(sparse, full);
            let Some(action) = c.legal_actions().into_iter().next() else {
                break;
            };
            c.apply(action).expect("a legal action");
        }
    }
}

#[test]
fn secondary_enemy_powers_are_minion() {
    for kind in PowerKind::ALL {
        assert_eq!(
            kind.owner_is_secondary_enemy(),
            kind == PowerKind::Minion,
            "{kind:?}: `Creature::is_secondary_enemy` compares the kind with Minion"
        );
    }
}

#[test]
fn card_id_rank_is_ordinal_id_order() {
    let mut by_id = CardKind::ALL.to_vec();
    by_id.sort_by(|a, b| a.entry().cmp(b.entry()));
    let mut by_rank = CardKind::ALL.to_vec();
    by_rank.sort_by_key(|k| k.id_rank());
    assert_eq!(by_id, by_rank);
}

/// The batch move equals moving the cards one by one ([`CombatState::move_to_pile_bottom`]).
#[test]
fn batch_pile_move_matches_one_by_one() {
    let c = new_combat();
    let all: Vec<CardId> = (c.state.player.hand.iter())
        .chain(&c.state.player.draw)
        .copied()
        .collect();
    assert!(
        all.len() >= 6,
        "the starter deck is in the hand and draw pile"
    );
    let mut base = c.state;
    // Spread the cards over the piles first.
    for (i, &card) in all.iter().enumerate() {
        let pile = [PileType::Draw, PileType::Discard, PileType::Exhaust][i % 3];
        base.remove_from_pile(card);
        base.insert_into_pile(card, pile, None);
    }
    let half: Vec<CardId> = all.iter().rev().step_by(2).copied().collect();
    // Whole piles (as a shuffle moves them), and every card.
    let whole: Vec<CardId> = (base.player.discard.iter())
        .chain(&base.player.draw)
        .copied()
        .collect();
    let every: Vec<CardId> = all.iter().rev().copied().collect();
    for target in [PileType::Draw, PileType::Discard, PileType::Exhaust]
        .into_iter()
        .flat_map(|t| [(t, &half), (t, &whole), (t, &every)])
    {
        let (target, batch) = target;
        let mut sequential = base.clone();
        let mut olds = Vec::new();
        for &card in batch {
            olds.push((card, sequential.remove_from_pile(card)));
            sequential.insert_into_pile(card, target, None);
        }
        let mut bulk = base.clone();
        let mut moved = MovedCards::new();
        assert!(
            bulk.move_to_pile_bottom(batch, target, &mut moved),
            "every card is in a pile once"
        );
        assert_eq!(moved.to_vec(), olds);
        assert!(bulk == sequential, "batch move to {target:?} differs");
    }
    let twice = [all[0], all[0]];
    assert!(
        !base
            .clone()
            .move_to_pile_bottom(&twice, PileType::Draw, &mut MovedCards::new())
    );
}
