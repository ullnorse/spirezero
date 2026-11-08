//! `DaggerThrow` (`Models.Cards/DaggerThrow.cs`): cost 1, 9 damage (12 upgraded), draw 1 card,
//! then discard 1.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct DaggerThrow;

#[sts2_macros::hooks]
impl Hooks for DaggerThrow {
    type Me = CardId;
}

impl CardModel for DaggerThrow {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Dagger Throw"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 9).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("DaggerThrow needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.draw(Decimal::ONE, false)?;
        // FromHandForDiscard(prefs: 1).FirstOrDefault(): exactly one card, if any.
        let options = eng.s.player.hand.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Discard), options, 1, 1)?
            .first()
        {
            eng.discard(&[card])?;
        }
        Ok(())
    }
}
