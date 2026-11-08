//! `PaelsGrowth` (`Models.Relics/PaelsGrowth.cs`): Ancient (Pael); `AfterObtained` enchants 1
//! chosen card with `Clone` 4 (`FromDeckForEnchantment`); every rest site has the `CLONE` option
//! (`CloneRestSiteOption`: each `Clone` deck card gets a copy).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::RelicModel;
use crate::run::{RestOption, RestOptionKind, RunCtx, RunError, RunState, SelectPurpose};

/// `CardSelectorPrefs(EnchantSelectionPrompt, 1)`.
const CARDS: usize = 1;
/// `CardCmd.Enchant<Clone>(item, 4m)`.
const AMOUNT: i32 = 4;

pub struct PaelsGrowth;

impl RelicModel for PaelsGrowth {
    fn name(&self) -> &'static str {
        "Pael's Growth"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let kind = EnchantmentKind::Clone;
        let options = ctx.state().enchantable_cards(kind)?;
        ctx.select(
            SelectPurpose::Enchant {
                kind,
                amount: AMOUNT,
            },
            options,
            CARDS,
        )
    }

    fn try_modify_rest_site_options(
        &self,
        _s: &RunState,
        _me: usize,
        options: &mut Vec<RestOption>,
    ) -> bool {
        options.push(RestOption {
            kind: RestOptionKind::Clone,
            enabled: true,
        });
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for PaelsGrowth {
    type Me = RelicId;
}
