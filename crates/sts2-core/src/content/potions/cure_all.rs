//! `CureAll` (`Models.Potions/CureAll.cs`): the target player gains 1 energy and draws 2 cards.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: i32 = 1;
/// `CardsVar(2)`.
const CARDS: i32 = 2;

pub struct CureAll;

#[sts2_macros::hooks]
impl Hooks for CureAll {
    type Me = PotionId;
}

impl PotionModel for CureAll {
    fn name(&self) -> &'static str {
        "Cure All"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        eng.gain_energy(Decimal::from(ENERGY))?;
        eng.draw(Decimal::from(CARDS), false)?;
        Ok(())
    }
}
