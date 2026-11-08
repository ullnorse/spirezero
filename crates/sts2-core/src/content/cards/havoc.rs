//! `Havoc` (`Models.Cards/Havoc.cs`): cost 1 (0 upgraded); auto-play the top card of the
//! draw pile, which is exhausted whatever it is (`forceExhaust`; the card itself has no Exhaust keyword, only a hover tip).

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};

pub struct Havoc;

#[sts2_macros::hooks]
impl Hooks for Havoc {
    type Me = CardId;
}

impl CardModel for Havoc {
    /// Auto-plays the top draw-pile card; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Havoc"
    }

    fn cost(&self) -> i32 {
        1
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

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        eng.auto_play_from_draw_pile(1, Position::Top, true)
    }
}
