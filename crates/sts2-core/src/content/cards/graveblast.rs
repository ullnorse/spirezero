//! `Graveblast` (`Models.Cards/Graveblast.cs`): cost 1, Exhaust (removed by the upgrade), 4
//! damage (6 upgraded), then return a card chosen from the discard pile to the hand.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Graveblast;

#[sts2_macros::hooks]
impl Hooks for Graveblast {
    type Me = CardId;
}

impl CardModel for Graveblast {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Graveblast"
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 4).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Graveblast needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // CardSelectCmd.FromCombatPile(Discard, CardSelectorPrefs(prompt, 1)).
        let options = eng.s.player.discard.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        {
            eng.add_to_pile(card, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}
