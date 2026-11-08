//! `Dredge` (`Models.Cards/Dredge.cs`): cost 1, Exhaust (upgrade adds Retain); choose up to 3
//! cards of the discard pile, as many as fit in the hand, and put them in the hand.

use crate::combat::commands::MAX_HAND_SIZE;
use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Dredge;

#[sts2_macros::hooks]
impl Hooks for Dredge {
    type Me = CardId;
}

impl CardModel for Dredge {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Dredge"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Retain);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).count_var_len(Var::Cards);
        let count = cards.min(MAX_HAND_SIZE.saturating_sub(eng.s.player.hand.len()));
        if count > 0 {
            // CardSelectCmd.FromCombatPile(Discard, CardSelectorPrefs(prompt, count)).
            let options = eng.s.player.discard.to_vec();
            let chosen = eng.choose_cards(
                Prompt::card(play.card, PromptText::Own),
                options,
                count,
                count,
            )?;
            eng.add_cards_to_pile(&chosen, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}
