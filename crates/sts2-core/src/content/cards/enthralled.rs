//! `Enthralled` (`Models.Cards/Enthralled.cs`): curse (`BloodSoakedRose`), cost 2, Eternal, no
//! effect of its own. While it is in hand, no other card can be played manually
//! (`ShouldPlay`).

use crate::combat::AutoPlayType;
use crate::combat::hooks::Hooks;
use crate::combat::state::PileType;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardKind, CardModel, CardRarity, CardType, Keywords, TargetType,
};

pub struct Enthralled;

#[sts2_macros::hooks]
impl Hooks for Enthralled {
    type Me = CardId;

    /// `ShouldPlay(card, autoPlayType)`: false for a manual play of any card but an
    /// `Enthralled` while this one is in hand (`card.Owner != Owner` never holds: one player);
    /// auto-plays go through.
    fn should_play(
        &self,
        s: &CombatState,
        me: CardId,
        card: CardId,
        auto_play: AutoPlayType,
    ) -> bool {
        s.pile_of(me) != Some(PileType::Hand)
            || s.card(card).kind == CardKind::Enthralled
            || auto_play != AutoPlayType::None
    }
}

impl CardModel for Enthralled {
    fn name(&self) -> &'static str {
        "Enthralled"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Curse
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Curse
    }

    fn target(&self) -> TargetType {
        TargetType::None
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Eternal)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
