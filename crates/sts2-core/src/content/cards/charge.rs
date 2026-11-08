//! `Charge` (`Models.Cards/Charge.cs`): Regent uncommon skill, cost 1; choose 2 cards of the draw
//! pile and transform them into `MinionDiveBomb`s, upgraded if this card is.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{
    CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var, pools,
};

pub struct Charge;

#[sts2_macros::hooks]
impl Hooks for Charge {
    type Me = CardId;
}

impl CardModel for Charge {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "CHARGE!!"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // CardSelectCmd.FromCombatPile(Draw, CardSelectorPrefs(TransformSelectionPrompt, Cards)).
        let options = eng.s.player.draw.to_vec();
        let count = eng.s.card(play.card).count_var_len(Var::Cards);
        let upgraded = eng.s.card(play.card).is_upgraded();
        for card in eng.choose_cards(
            Prompt::card(play.card, PromptText::Transform),
            options,
            count,
            count,
        )? {
            pools::transform_to_upgraded(eng, card, CardKind::MinionDiveBomb, upgraded)?;
        }
        Ok(())
    }
}
