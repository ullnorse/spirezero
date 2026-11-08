//! `Uproar` (`Models.Cards/Uproar.cs`): Defect common attack, cost 2; 2 hits of 6 damage (8
//! upgraded), then auto-play a random Attack of the draw pile with no target: a playable one
//! (`StableShuffle` on `Rng.Shuffle`), else any Attack. An `AnyEnemy` one picks its target on
//! `Rng.CombatTargets`.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    self, CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::rng::RunRngType;
use crate::shuffle::stable_shuffle_by;

pub struct Uproar;

/// The first of the draw pile's Attacks (only the non-Unplayable ones if `playable_only`), after
/// a `StableShuffle` on `Rng.Shuffle`.
fn pick_attack(eng: &mut Engine<'_>, playable_only: bool) -> Option<CardId> {
    let hooked = eng.hooked();
    let mut attacks: Vec<CardId> = hooked
        .s
        .player
        .draw
        .iter()
        .copied()
        .filter(|&c| {
            let card = hooked.s.card(c);
            card.card_type() == CardType::Attack
                && !(playable_only && hooked.has_keyword(c, CardKeyword::Unplayable))
        })
        .collect();
    let state = &mut *eng.s;
    let all_cards = &state.cards;
    stable_shuffle_by(&mut attacks, &mut state.rng[RunRngType::Shuffle], |a, b| {
        cards::compare(&all_cards[a.index()], &all_cards[b.index()])
    });
    attacks.first().copied()
}

#[sts2_macros::hooks]
impl Hooks for Uproar {
    type Me = CardId;
}

impl CardModel for Uproar {
    /// Plays a random Attack from the draw pile; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Uproar"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 6).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Uproar needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(2),
        )?;
        let picked = pick_attack(eng, true).or_else(|| pick_attack(eng, false));
        if let Some(card) = picked {
            eng.auto_play(card, None)?;
        }
        Ok(())
    }
}
