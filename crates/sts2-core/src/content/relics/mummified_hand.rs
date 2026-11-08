//! `MummifiedHand` (`Models.Relics/MummifiedHand.cs`): after the owner plays a Power, a random
//! card in the hand costs 0 this turn (preferring cards that cost energy).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, RelicId};
use crate::content::cards::CardType;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;

pub struct MummifiedHand;

impl RelicModel for MummifiedHand {
    fn name(&self) -> &'static str {
        "Mummified Hand"
    }
}

#[sts2_macros::hooks]
impl Hooks for MummifiedHand {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, _me: RelicId, play: &CardPlay) -> Flow {
        // The only player owns every card.
        if !eng.s.in_progress || eng.s.card(play.card).card_type() != CardType::Power {
            return Ok(());
        }
        let hand = eng.s.player.hand.to_vec();
        // GetWithModifiers(None) > 0 || BaseStarCost > 0.
        let list: Vec<CardId> = hand
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).energy_cost.base > 0 || eng.s.card(c).star_cost.base > 0)
            .collect();
        let costing_list: Vec<CardId> = list
            .iter()
            .copied()
            .filter(|&c| eng.hooked().costs_energy_or_stars(c, true))
            .collect();
        let costing_hand: Vec<CardId> = hand
            .iter()
            .copied()
            .filter(|&c| eng.hooked().costs_energy_or_stars(c, true))
            .collect();
        let rng = &mut eng.s.rng[RunRngType::CombatCardSelection];
        let card = rng
            .next_item(&costing_list)
            .or_else(|| rng.next_item(&costing_hand))
            .or_else(|| rng.next_item(&list))
            .or_else(|| rng.next_item(&hand))
            .copied();
        if let Some(card) = card {
            // SetToFreeThisTurn.
            eng.s.set_to_free_this_turn(card);
        }
        Ok(())
    }
}
