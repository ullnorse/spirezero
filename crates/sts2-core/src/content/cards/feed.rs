//! `Feed` (`Models.Cards/Feed.cs`): cost 1, Exhaust, 10 damage (12 upgraded); if that kills the target (and its powers allow Fatal) gain 3 max HP (4 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Feed;

#[sts2_macros::hooks]
impl Hooks for Feed {
    type Me = CardId;
}

impl CardModel for Feed {
    fn name(&self) -> &'static str {
        "Feed"
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 10).upgrade(2),
            CardVar::new(Var::MaxHp, 3).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Feed needs a target");
        let should_trigger_fatal = eng.target_allows_fatal(target);
        let player = eng.s.player.creature;
        let results = eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(eng.s.card(play.card).var(Var::Damage)),
            Some(play.card),
        ))?;
        if should_trigger_fatal && Engine::fatal_triggered(&results) {
            eng.gain_max_hp(player, eng.s.card(play.card).var(Var::MaxHp))?;
        }
        Ok(())
    }
}
