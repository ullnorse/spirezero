//! `Rainbow` (`Models.Cards/Rainbow.cs`): cost 2, Exhaust (upgrade removes it); channel Lightning, Frost and Dark.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct Rainbow;

#[sts2_macros::hooks]
impl Hooks for Rainbow {
    type Me = CardId;
}

impl CardModel for Rainbow {
    fn name(&self) -> &'static str {
        "Rainbow"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
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

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        eng.channel(OrbKind::Lightning)?;
        eng.channel(OrbKind::Frost)?;
        eng.channel(OrbKind::Dark)
    }
}
