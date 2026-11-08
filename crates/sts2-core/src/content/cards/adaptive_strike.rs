//! `AdaptiveStrike` (`Models.Cards/AdaptiveStrike.cs`): cost 2, 18 damage (23 upgraded), then a clone of itself
//! with cost 0 for the combat goes into the discard pile.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct AdaptiveStrike;

#[sts2_macros::hooks]
impl Hooks for AdaptiveStrike {
    type Me = CardId;
}

impl CardModel for AdaptiveStrike {
    fn name(&self) -> &'static str {
        "Adaptive Strike"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 18).upgrade(5)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("AdaptiveStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let clone = eng.create_clone(play.card);
        eng.s.energy_cost_mut(clone).set_this_combat(0);
        eng.add_generated_card_to_combat(clone, PileType::Discard, Some(player))?;
        Ok(())
    }
}
