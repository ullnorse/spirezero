//! `Transfigure` (`Models.Cards/Transfigure.cs`): rare skill, cost 1, Exhaust (removed by the
//! upgrade); choose a hand card: it costs 1 more for the rest of the combat (unless it costs X
//! or has a negative cost) and replays once more each time it is played.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Transfigure;

#[sts2_macros::hooks]
impl Hooks for Transfigure {
    type Me = CardId;
}

impl CardModel for Transfigure {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Transfigure"
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
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // CardSelectCmd.FromHand(prefs 1, no filter).
        let options = eng.s.player.hand.to_vec();
        for card in eng.choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)? {
            // `!CostsX && GetWithModifiers(CostModifiers.None) >= 0`: the base cost.
            let cost = eng.s.card(card).energy_cost;
            if !cost.costs_x && cost.base >= 0 {
                eng.s.energy_cost_mut(card).add_this_combat(1);
            }
            eng.s.card_mut(card).base_replay_count += 1;
        }
        Ok(())
    }
}
