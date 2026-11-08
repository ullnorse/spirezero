//! `NeowsFury` (`Models.Cards/NeowsFury.cs`): cost 1, Exhaust; 10 damage (upgraded 14) to one
//! enemy, then choose up to 2 (upgraded 3) cards of the discard pile, as many as fit in the
//! hand, and put them in the hand.

use crate::combat::commands::MAX_HAND_SIZE;
use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

/// Ancient card added by `NeowsTorment`.
pub struct NeowsFury;

#[sts2_macros::hooks]
impl Hooks for NeowsFury {
    type Me = CardId;
}

impl CardModel for NeowsFury {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Neow's Fury"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 10).upgrade(4),
            CardVar::new(Var::Cards, 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("NeowsFury needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let cards = eng.s.card(play.card).count_var_len(Var::Cards);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let count = cards.min(MAX_HAND_SIZE.saturating_sub(eng.s.player.hand.len()));
        if count > 0 {
            // `CardSelectCmd.FromCombatPile(Discard, CardSelectorPrefs(prompt, 0, count))`.
            let options = eng.s.player.discard.to_vec();
            let chosen =
                eng.choose_cards(Prompt::card(play.card, PromptText::Own), options, 0, count)?;
            eng.add_cards_to_pile(&chosen, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}
