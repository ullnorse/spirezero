//! `DecisionsDecisions` (`Models.Cards/DecisionsDecisions.cs`): Regent rare skill, cost 0 and 6
//! stars, Exhaust; draw 3 cards (5 upgraded), choose a non-Unplayable Skill in the hand and
//! auto-play it `Repeat` (3) times.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct DecisionsDecisions;

#[sts2_macros::hooks]
impl Hooks for DecisionsDecisions {
    type Me = CardId;
}

impl CardModel for DecisionsDecisions {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Decisions, Decisions"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn star_cost(&self) -> i32 {
        6
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Cards, 3).upgrade(2),
            CardVar::new(Var::Repeat, 3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).var(Var::Cards);
        eng.draw(cards, false)?;
        // CardSelectCmd.FromHand(CardSelectorPrefs(prompt, 1), Skill && !Unplayable).
        let hooked = eng.hooked();
        let options: Vec<_> = hooked
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| {
                hooked.s.card(c).card_type() == CardType::Skill
                    && !hooked.has_keyword(c, CardKeyword::Unplayable)
            })
            .collect();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        {
            for _ in 0..eng.s.card(play.card).count_var(Var::Repeat) {
                eng.auto_play(card, None)?;
            }
        }
        Ok(())
    }
}
