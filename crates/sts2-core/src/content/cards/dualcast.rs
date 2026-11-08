//! `Dualcast` (`Models.Cards/Dualcast.cs`): cost 1 (0 upgraded); evoke the front orb twice
//! (`EvokeNext(dequeue: false)`, then `EvokeNext`).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};

pub struct Dualcast;

#[sts2_macros::hooks]
impl Hooks for Dualcast {
    type Me = CardId;
}

impl CardModel for Dualcast {
    fn name(&self) -> &'static str {
        "Dualcast"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Basic
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        if !eng.s.player.orbs.is_empty() {
            eng.evoke_next(false)?;
            eng.evoke_next(true)?;
        }
        Ok(())
    }
}
