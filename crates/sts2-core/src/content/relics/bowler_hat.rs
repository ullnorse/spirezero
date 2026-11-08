//! `BowlerHat` (`Models.Relics/BowlerHat.cs`): gold gained is multiplied by 1.25 (then truncated
//! by `GainGold`). Its `AfterModifyingGoldGained` only flashes. Combat gold is not ported (no
//! in-scope combat effect gains gold).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RunState;
use crate::run::relic_bag::is_before_act3_treasure_chest;

/// `DynamicVar("GoldIncrease", 1.25m)`.
const GOLD_INCREASE: Decimal = Decimal::new(125, 2);

pub struct BowlerHat;

impl RelicModel for BowlerHat {
    /// `IsAllowed` (`BowlerHat.cs:20-23`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Bowler Hat"
    }

    fn modify_gold_gained_out_of_combat(
        &self,
        _s: &RunState,
        _me: usize,
        amount: Decimal,
    ) -> Decimal {
        amount * GOLD_INCREASE
    }
}

#[sts2_macros::hooks]
impl Hooks for BowlerHat {
    type Me = RelicId;
}
