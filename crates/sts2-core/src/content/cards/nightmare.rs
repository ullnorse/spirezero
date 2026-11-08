//! `Nightmare` (`Models.Cards/Nightmare.cs`): cost 3 (2 upgraded), Exhaust; picks a card from the
//! hand and applies `NightmarePower` (3), which adds 3 clones of it to the hand next turn.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};
use crate::content::powers::{PowerKind, nightmare::Nightmare as NightmarePower};
use crate::decimal::Decimal;

pub struct Nightmare;

#[sts2_macros::hooks]
impl Hooks for Nightmare {
    type Me = CardId;
}

impl CardModel for Nightmare {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Nightmare"
    }

    fn cost(&self) -> i32 {
        3
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
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // FromHand(prefs: 1).FirstOrDefault(): exactly one card, if any.
        let options = eng.s.player.hand.to_vec();
        let selected = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
            .copied();
        let Some(selected) = selected else {
            return Ok(());
        };
        let player = eng.s.player.creature;
        let power = eng.apply_power_instance_from_card(
            Some(play.card),
            player,
            PowerKind::Nightmare,
            Decimal::from(3),
            Some(player),
        )?;
        if let Some(me) = power {
            NightmarePower::set_selected_card(eng, me, selected);
        }
        Ok(())
    }
}
