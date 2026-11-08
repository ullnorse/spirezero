//! `Glimmer` (`Models.Cards/Glimmer.cs`): Regent uncommon skill, cost 1; draw 3 cards (4
//! upgraded), then put 1 card from the hand on top of the draw pile.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Glimmer;

#[sts2_macros::hooks]
impl Hooks for Glimmer {
    type Me = CardId;
}

impl CardModel for Glimmer {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Glimmer"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Cards, 3).upgrade(1),
            CardVar::new(Var::Named("PutBack"), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).var(Var::Cards);
        eng.draw(cards, false)?;
        // CardSelectCmd.FromHand(CardSelectorPrefs(prompt, PutBack)).
        let put_back = eng.s.card(play.card).count_var_len(Var::Named("PutBack"));
        let options = eng.s.player.hand.to_vec();
        for card in eng.choose_cards(
            Prompt::card(play.card, PromptText::Own),
            options,
            put_back,
            put_back,
        )? {
            eng.add_to_pile(card, PileType::Draw, Position::Top)?;
        }
        Ok(())
    }
}
