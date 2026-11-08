//! `Cleanse` (`Models.Cards/Cleanse.cs`): cost 1, Summon 3 (5 upgraded), then exhaust a card
//! chosen from the draw pile.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Cleanse;

#[sts2_macros::hooks]
impl Hooks for Cleanse {
    type Me = CardId;
}

impl CardModel for Cleanse {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Cleanse"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Summon, 3).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.summon_osty(eng.s.card(play.card).var(Var::Summon))?;
        // CardSelectCmd.FromCombatPile(Draw, CardSelectorPrefs(ExhaustSelectionPrompt, 1)).
        let options = eng.s.player.draw.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Exhaust), options, 1, 1)?
            .first()
        {
            eng.exhaust(card, false)?;
        }
        Ok(())
    }
}
