//! `DragonFruit` (`Models.Relics/DragonFruit.cs`): whenever the owner gains gold, 1 max HP.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::RunState;
use crate::run::relic_bag::is_before_act3_treasure_chest;

/// `MaxHpVar(1m)`.
const MAX_HP: i32 = 1;

pub struct DragonFruit;

impl RelicModel for DragonFruit {
    /// `IsAllowed` (`DragonFruit.cs:17-20`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Dragon Fruit"
    }

    fn after_gold_gained_out_of_combat(&self, s: &mut RunState, _me: usize) {
        s.gain_max_hp(MAX_HP);
    }
}

#[sts2_macros::hooks]
impl Hooks for DragonFruit {
    type Me = RelicId;
}
