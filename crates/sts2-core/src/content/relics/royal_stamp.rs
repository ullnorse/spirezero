//! `RoyalStamp` (`Models.Relics/RoyalStamp.cs`): `AfterObtained` enchants 1 chosen deck card
//! `RoyallyApproved` can enchant. The candidates are shuffled on `Rng.Niche` first, but the
//! selection lists them in deck order again (`FromDeckForEnchantment` sorts by deck index), so
//! the shuffle only moves the stream.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::{RunCtx, RunError, SelectPurpose};
use crate::shuffle::unstable_shuffle;

pub struct RoyalStamp;

impl RelicModel for RoyalStamp {
    fn name(&self) -> &'static str {
        "Royal Stamp"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let mut options = s.enchantable_cards(EnchantmentKind::RoyallyApproved)?;
        unstable_shuffle(&mut options, &mut s.rng[RunRngType::Niche]);
        options.sort_unstable();
        ctx.select(
            SelectPurpose::Enchant {
                kind: EnchantmentKind::RoyallyApproved,
                amount: 1,
            },
            options,
            1,
        )
    }
}

#[sts2_macros::hooks]
impl Hooks for RoyalStamp {
    type Me = RelicId;
}
