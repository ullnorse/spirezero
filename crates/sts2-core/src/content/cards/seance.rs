//! `Seance` (`Models.Cards/Seance.cs`): rare skill, cost 1 (0 upgraded), Ethereal; choose 1 card
//! of the draw pile and transform it into a `Soul`.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardKind, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Seance;

#[sts2_macros::hooks]
impl Hooks for Seance {
    type Me = CardId;
}

impl CardModel for Seance {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Seance"
    }

    fn cost(&self) -> i32 {
        1
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
        Keywords::NONE.with(CardKeyword::Ethereal)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // CardSelectCmd.FromCombatPile(Draw, CardSelectorPrefs(prompt, 1)).
        let options = eng.s.player.draw.to_vec();
        let count = eng.s.card(play.card).count_var_len(Var::Cards);
        for card in eng.choose_cards(
            Prompt::card(play.card, PromptText::Transform),
            options,
            count,
            count,
        )? {
            eng.transform_to(card, CardKind::Soul)?;
        }
        Ok(())
    }
}
