//! `TheHunt` (`Models.Cards/TheHunt.cs`): cost 1, Attack, Rare, Exhaust; 10 damage (15
//! upgraded). If that kills the target (and none of its powers vetoes
//! `ShouldOwnerDeathTriggerFatal`), the room offers an extra 3-card reward
//! (`CombatRoom.AddExtraReward`, [`ExtraReward::Card`]) and the owner gains `TheHuntPower`.
//! `CanBeGeneratedInCombat` is false.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, ExtraReward, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct TheHunt;

#[sts2_macros::hooks]
impl Hooks for TheHunt {
    type Me = CardId;
}

impl CardModel for TheHunt {
    fn name(&self) -> &'static str {
        "The Hunt"
    }

    fn cost(&self) -> i32 {
        1
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 10).upgrade(5)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("TheHunt needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let should_trigger_fatal = eng.target_allows_fatal(target);
        let player = eng.s.player.creature;
        let results = eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        if should_trigger_fatal && Engine::fatal_triggered(&results) {
            eng.s.extra_rewards.push(ExtraReward::Card(3));
            eng.apply_power_from_card(
                Some(play.card),
                player,
                PowerKind::TheHunt,
                Decimal::ONE,
                Some(player),
            )?;
        }
        Ok(())
    }
}
