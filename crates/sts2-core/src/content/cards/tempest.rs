//! `Tempest` (`Models.Cards/Tempest.cs`): cost X; channel X Lightning (X + 1 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};

pub struct Tempest;

#[sts2_macros::hooks]
impl Hooks for Tempest {
    type Me = CardId;
}

impl CardModel for Tempest {
    fn name(&self) -> &'static str {
        "Tempest"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn has_energy_cost_x(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // X Lightning, one more upgraded.
        let upgraded = eng.s.card(play.card).is_upgraded();
        let count = eng.hooked().resolve_energy_x_value(play.card) + i32::from(upgraded);
        for _ in 0..count {
            eng.channel(OrbKind::Lightning)?;
        }
        Ok(())
    }
}
