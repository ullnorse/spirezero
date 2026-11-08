//! `Cauldron` (`Models.Relics/Cauldron.cs`): `AfterObtained` offers 5 potion rewards
//! (`RewardsCmd.OfferCustom`). In test mode (the referee) they are fixed: Flex, Weak, Vulnerable,
//! Strength and Dexterity (`docs/specs/test-mode.md`); otherwise `PotionReward(Owner)` rolls each
//! on `rewards`.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{PendingReward, Reward, RunCtx, RunError};

/// `DynamicVar("Potions", 5m)`.
const POTIONS: usize = 5;

/// `_testPotions`.
const TEST_POTIONS: [&str; POTIONS] = [
    "FLEX_POTION",
    "WEAK_POTION",
    "VULNERABLE_POTION",
    "STRENGTH_POTION",
    "DEXTERITY_POTION",
];

pub struct Cauldron;

impl RelicModel for Cauldron {
    fn name(&self) -> &'static str {
        "Cauldron"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let rewards = if ctx.state().test_mode {
            (0..POTIONS)
                .map(|i| PendingReward::Ready(Reward::Potion(TEST_POTIONS[i % POTIONS])))
                .collect()
        } else {
            vec![PendingReward::Potion; POTIONS]
        };
        ctx.offer_pending_rewards(rewards);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Cauldron {
    type Me = RelicId;
}
