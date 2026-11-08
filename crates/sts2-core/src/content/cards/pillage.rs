//! `Pillage` (`Models.Cards/Pillage.cs`): cost 1, 6 damage (9 upgraded), then draws cards until a non-Attack is drawn or the hand is full.
//!
//! The draw loop is bounded by [`LOOP_GUARD`]: with Hellraiser auto-playing (or a `ShouldPlay` veto discarding) every Strike drawn and only Strikes left to draw, the game loops forever; we stop instead.

use crate::combat::commands::MAX_HAND_SIZE;
use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

/// Maximum draws in one Pillage. It only cuts a loop the game never leaves: each iteration of a
/// terminating sequence either grows the hand (so at most `MAX_HAND_SIZE` of them) or
/// auto-plays a Strike into the enemies, so it ends far sooner.
const LOOP_GUARD: usize = 10_000;

pub struct Pillage;

#[sts2_macros::hooks]
impl Hooks for Pillage {
    type Me = CardId;
}

impl CardModel for Pillage {
    fn name(&self) -> &'static str {
        "Pillage"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 6).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Pillage needs a target");
        let player = eng.s.player.creature;
        let damage = eng.s.card(play.card).var(Var::Damage);
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // `do { card = Draw(); } while (card != null && card.Type == Attack && hand < MaxCardsInHand)`.
        for _ in 0..LOOP_GUARD {
            let drawn = eng.draw(Decimal::from(1), false)?;
            let again = drawn.first().is_some_and(|&c| {
                eng.s.card(c).card_type() == CardType::Attack
                    && eng.s.player.hand.len() < MAX_HAND_SIZE
            });
            if !again {
                return Ok(());
            }
        }
        Ok(())
    }
}
