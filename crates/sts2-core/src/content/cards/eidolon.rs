//! `Eidolon` (`Models.Cards/Eidolon.cs`): cost 2 (1 upgraded), Exhaust; autoplay every
//! Ethereal, playable card in the exhaust pile.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct Eidolon;

#[sts2_macros::hooks]
impl Hooks for Eidolon {
    type Me = CardId;
}

impl CardModel for Eidolon {
    /// Auto-plays the cards it picks; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Eidolon"
    }

    fn cost(&self) -> i32 {
        2
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

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        // CreatureCmd.TriggerAnim is visual only. The list is taken before any autoplay.
        let hooked = eng.hooked();
        let cards: Vec<CardId> = hooked
            .s
            .player
            .exhaust
            .iter()
            .copied()
            .filter(|&c| {
                hooked.has_keyword(c, CardKeyword::Ethereal)
                    && !hooked.has_keyword(c, CardKeyword::Unplayable)
            })
            .collect();
        for card in cards {
            eng.auto_play(card, None)?;
        }
        Ok(())
    }
}
