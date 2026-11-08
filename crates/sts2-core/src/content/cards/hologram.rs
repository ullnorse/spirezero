//! `Hologram` (`Models.Cards/Hologram.cs`): cost 1, Exhaust (removed by the upgrade), 3 block (5 upgraded);
//! pick a card of the discard pile (`CardSelectCmd.FromCombatPile`, exactly one) and put it into the hand.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText, ValueProps};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Hologram;

#[sts2_macros::hooks]
impl Hooks for Hologram {
    type Me = CardId;
}

impl CardModel for Hologram {
    fn name(&self) -> &'static str {
        "Hologram"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 3).upgrade(2)];
        VARS
    }

    fn may_prompt(&self) -> bool {
        true
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
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
