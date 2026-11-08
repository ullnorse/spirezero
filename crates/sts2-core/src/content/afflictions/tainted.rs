//! `Tainted` (`Models.Afflictions/Tainted.cs`): afflicts Skills only and stacks; its logic is
//! `VitalSparkPower`'s (a played `Tainted` card applies `TaintedPower`).

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::content::afflictions::AfflictionModel;
use crate::content::cards::CardType;

/// `VitalSparkPower`'s affliction on the player's Skills.
pub struct Tainted;

impl AfflictionModel for Tainted {
    fn name(&self) -> &'static str {
        "Tainted"
    }

    fn can_afflict_card_type(&self, card_type: CardType) -> bool {
        card_type == CardType::Skill
    }

    fn is_stackable(&self) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Tainted {
    type Me = CardId;
}
