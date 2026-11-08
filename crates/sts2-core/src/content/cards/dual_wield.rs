//! `DualWield` (`Models.Cards/DualWield.cs`): Event skill, cost 1; pick an Attack or Power in the
//! hand and add 1 clone of it (2 upgraded) to the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct DualWield;

#[sts2_macros::hooks]
impl Hooks for DualWield {
    type Me = CardId;
}

impl CardModel for DualWield {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Dual Wield"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let options: Vec<CardId> = eng
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| {
                matches!(
                    eng.s.card(c).card_type(),
                    CardType::Attack | CardType::Power
                )
            })
            .collect();
        let Some(&selection) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        else {
            return Ok(());
        };
        let player = eng.s.player.creature;
        for _ in 0..eng.s.card(play.card).count_var(Var::Cards) {
            let clone = eng.create_clone(selection);
            eng.add_generated_card_to_combat(clone, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
