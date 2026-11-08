//! `GhostSeed` (`Models.Relics/GhostSeed.cs`): the owner's basic Strikes and Defends are
//! Ethereal, as they enter combat and when a fight starts.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState, Engine, Flow, PileType, RelicId};
use crate::content::cards::{CardKeyword, CardRarity};
use crate::content::relics::RelicModel;

pub struct GhostSeed;

impl RelicModel for GhostSeed {
    fn name(&self) -> &'static str {
        "Ghost Seed"
    }
}

/// `CanAffect`: a Basic card tagged Strike or Defend (the starter Strike and Defend), not
/// already locally Ethereal.
fn can_affect(s: &CombatState, card: CardId) -> bool {
    let card = s.card(card);
    card.kind.rarity() == CardRarity::Basic
        && (card.kind.is_strike() || card.kind.is_defend())
        && !card.keywords.contains(CardKeyword::Ethereal)
}

#[sts2_macros::hooks]
impl Hooks for GhostSeed {
    type Me = RelicId;

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, _me: RelicId, card: CardId) -> Flow {
        // The only player owns every card.
        if can_affect(eng.s, card) {
            eng.apply_keyword(card, CardKeyword::Ethereal);
        }
        Ok(())
    }

    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // PlayerCombatState.AllCards.
        let all: Vec<CardId> = PileType::LISTENER_ORDER
            .into_iter()
            .flat_map(|p| eng.s.player.pile(p).iter().copied())
            .collect();
        for card in all {
            if can_affect(eng.s, card) {
                eng.apply_keyword(card, CardKeyword::Ethereal);
            }
        }
        Ok(())
    }
}
