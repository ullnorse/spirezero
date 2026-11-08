//! `DistilledChaos` (`Models.Potions/DistilledChaos.cs`): auto-play the top 3 cards of the
//! target player's draw pile.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};

/// `RepeatVar(3)`.
const REPEAT: u32 = 3;

pub struct DistilledChaos;

#[sts2_macros::hooks]
impl Hooks for DistilledChaos {
    type Me = PotionId;
}

impl PotionModel for DistilledChaos {
    /// Auto-plays draw-pile cards; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Distilled Chaos"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // CardPileCmd.AutoPlayFromDrawPile(target.Player, Repeat, Top, forceExhaust: false).
        eng.auto_play_from_draw_pile(REPEAT, Position::Top, false)
    }
}
