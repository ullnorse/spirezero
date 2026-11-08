//! `CosmicIndifference` (`Models.Cards/CosmicIndifference.cs`): Regent common skill, cost 1; gain 6
//! block (9 upgraded), then put 1 card from the discard pile on top of the draw pile.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct CosmicIndifference;

#[sts2_macros::hooks]
impl Hooks for CosmicIndifference {
    type Me = CardId;
}

impl CardModel for CosmicIndifference {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Cosmic Indifference"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 6).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        // CardSelectCmd.FromCombatPile(Discard, CardSelectorPrefs(prompt, 1)).FirstOrDefault().
        let discard = eng.s.player.discard.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), discard, 1, 1)?
            .first()
            && matches!(
                eng.s.pile_of(card),
                Some(PileType::Draw | PileType::Discard)
            )
        {
            eng.add_to_pile(card, PileType::Draw, Position::Top)?;
        }
        Ok(())
    }
}
