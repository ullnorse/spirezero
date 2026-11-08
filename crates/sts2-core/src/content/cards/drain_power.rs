//! `DrainPower` (`Models.Cards/DrainPower.cs`): 10 damage (12 upgraded), then upgrades 2 random
//! upgradable cards in the discard pile (3 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::rng::RunRngType;

pub struct DrainPower;

#[sts2_macros::hooks]
impl Hooks for DrainPower {
    type Me = CardId;
}

impl CardModel for DrainPower {
    fn name(&self) -> &'static str {
        "Drain Power"
    }

    fn cost(&self) -> i32 {
        1
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 10).upgrade(2),
            CardVar::new(Var::Cards, 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("DrainPower needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let cards = eng.s.card(play.card).count_var_len(Var::Cards);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // `Where(IsUpgradable).TakeRandom(n, Rng.CombatCardSelection)`: the whole list is
        // shuffled eagerly (`UnstableShuffle`), then the first `n` are taken.
        let mut upgradable: Vec<CardId> = eng
            .s
            .player
            .discard
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).is_upgradable())
            .collect();
        eng.s.rng[RunRngType::CombatCardSelection].shuffle(&mut upgradable);
        for card in upgradable.into_iter().take(cards) {
            // CardCmd.Preview is UI only.
            eng.upgrade_card(card);
        }
        Ok(())
    }
}
