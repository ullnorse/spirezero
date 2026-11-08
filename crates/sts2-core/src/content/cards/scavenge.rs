//! `Scavenge` (`Models.Cards/Scavenge.cs`): Defect uncommon skill, cost 1; exhaust a
//! card of the hand (the player's pick), then gain 2 energy next turn (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Scavenge;

#[sts2_macros::hooks]
impl Hooks for Scavenge {
    type Me = CardId;
}

impl CardModel for Scavenge {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Scavenge"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // CardSelectCmd.FromHand(CardSelectorPrefs(ExhaustSelectionPrompt, 1)).FirstOrDefault().
        let options = eng.s.player.hand.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Exhaust), options, 1, 1)?
            .first()
        {
            eng.exhaust(card, false)?;
        }
        let player = eng.s.player.creature;
        let energy = eng.s.card(play.card).int_var(Var::Energy);
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::EnergyNextTurn,
            Decimal::from(energy),
            Some(player),
        )
    }
}
