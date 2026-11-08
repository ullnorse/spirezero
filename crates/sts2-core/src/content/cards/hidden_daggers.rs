//! `HiddenDaggers` (`Models.Cards/HiddenDaggers.cs`): cost 0, Skill, Uncommon; discards 2 cards
//! of the player's choice, then creates 2 Shivs (upgraded when this card is).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::shiv;
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct HiddenDaggers;

#[sts2_macros::hooks]
impl Hooks for HiddenDaggers {
    type Me = CardId;
}

impl CardModel for HiddenDaggers {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Hidden Daggers"
    }

    fn cost(&self) -> i32 {
        0
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
            CardVar::new(Var::Cards, 2),
            CardVar::new(Var::Named("Shivs"), 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // FromHandForDiscard(CardSelectorPrefs(prompt, 2), filter null): exactly 2, if any.
        let options = eng.s.player.hand.to_vec();
        let count = eng.s.card(play.card).count_var_len(Var::Cards);
        let cards = eng.choose_cards(
            Prompt::card(play.card, PromptText::Discard),
            options,
            count,
            count,
        )?;
        eng.discard(&cards)?;
        let shivs = eng.s.card(play.card).count_var_len(Var::Named("Shivs"));
        let shivs = shiv::create_in_hand(eng, shivs)?;
        if eng.s.card(play.card).is_upgraded() {
            for card in shivs {
                eng.upgrade_card(card);
            }
        }
        Ok(())
    }
}
