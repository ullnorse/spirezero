//! `SneckoOil` (`Models.Potions/SneckoOil.cs`): draw 7, then every card in the hand that does
//! not cost X and has a cost costs a random 0-3 this turn or until played.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;
use crate::rng::RunRngType;

/// `CardsVar(7)`.
const CARDS: i32 = 7;
/// `Rng.CombatEnergyCosts.NextInt(4)`.
const COST_BOUND: i32 = 4;

pub struct SneckoOil;

#[sts2_macros::hooks]
impl Hooks for SneckoOil {
    type Me = PotionId;
}

impl PotionModel for SneckoOil {
    fn name(&self) -> &'static str {
        "Snecko Oil"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        eng.draw(Decimal::from(CARDS), false)?;
        // Hand.Where(c => !c.EnergyCost.CostsX); GetWithModifiers(None) is the base (or the
        // negative base of an unplayable card), so `>= 0` means "has a base cost".
        for card in eng.s.player.hand.clone() {
            let cost = &eng.s.card(card).energy_cost;
            if !cost.costs_x && cost.base >= 0 {
                let next = eng.s.rng[RunRngType::CombatEnergyCosts].next_int(COST_BOUND);
                eng.s
                    .energy_cost_mut(card)
                    .set_this_turn_or_until_played(next);
            }
        }
        Ok(())
    }
}
