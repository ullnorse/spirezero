//! `SoldiersStew` (`Models.Potions/SoldiersStew.cs`): every Strike card in the target player's
//! combat piles gets one more replay.

use crate::combat::hooks::Hooks;
use crate::combat::state::PileType;
use crate::combat::{CardId, CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};

pub struct SoldiersStew;

#[sts2_macros::hooks]
impl Hooks for SoldiersStew {
    type Me = PotionId;
}

impl PotionModel for SoldiersStew {
    fn name(&self) -> &'static str {
        "Soldier's Stew"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _target = target.expect("AssertValidForTargetedPotion");
        // `AllCards.Where(c => c.Tags.Contains(CardTag.Strike))`.
        let strikes: Vec<CardId> = PileType::LISTENER_ORDER
            .into_iter()
            .flat_map(|p| eng.s.player.pile(p).iter().copied())
            .filter(|&c| eng.s.card(c).kind.is_strike())
            .collect();
        for card in strikes {
            eng.s.card_mut(card).base_replay_count += 1;
        }
        Ok(())
    }
}
