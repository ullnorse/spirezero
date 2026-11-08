//! `HandOfGreed` (`Models.Cards/HandOfGreed.cs`): cost 2; 20 damage (25 upgraded); if that kills
//! the target (and none of its powers vetoes `ShouldOwnerDeathTriggerFatal`), gain 20 gold (25
//! upgraded). Cannot be generated in combat.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct HandOfGreed;

#[sts2_macros::hooks]
impl Hooks for HandOfGreed {
    type Me = CardId;
}

impl CardModel for HandOfGreed {
    fn name(&self) -> &'static str {
        "Hand of Greed"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 20).upgrade(5),
            CardVar::new(Var::Gold, 20).upgrade(5),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("HandOfGreed needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let gold = Decimal::from(eng.s.card(play.card).int_var(Var::Gold));
        let should_trigger_fatal = eng.target_allows_fatal(target);
        let player = eng.s.player.creature;
        let results = eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        if should_trigger_fatal && Engine::fatal_triggered(&results) {
            eng.gain_gold(gold)?;
        }
        Ok(())
    }
}
