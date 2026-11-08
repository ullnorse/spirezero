//! `Claws` (`Models.Relics/Claws.cs`): Ancient (Tanx); `AfterObtained` lets the player pick up to
//! 6 transformable deck cards (`FromDeckForTransformation`, `MinSelect` 0, manual confirmation)
//! and transforms each into a `Maul`, upgraded if the original was and enchanted with the
//! original's enchantment if `Maul` can take it. `CardCmd.Transform` (the picks leave the deck,
//! the replacements join as [`RunState::transform_many`] orders them) draws no `Rng`: the
//! replacements are built beforehand.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError, RunState, SelectPurpose};

/// `CardsVar(6)`.
const CARDS: usize = 6;

pub struct Claws;

/// `CreateMaulFromOriginal(original, forPreview: false)`.
fn maul_from(original: RunCard) -> RunCard {
    let mut maul = RunCard::new("MAUL");
    if original.upgrade_level > 0 && maul.is_upgradable() {
        maul.upgrade();
    }
    if let Some(enchantment) = original.enchantment
        && maul.can_enchant(enchantment.kind) == Ok(true)
    {
        maul.enchantment = Some(enchantment);
        maul.calculated_level = None;
    }
    maul
}

impl RelicModel for Claws {
    fn name(&self) -> &'static str {
        "Claws"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, me: usize) -> Result<(), RunError> {
        let options = ctx.state().transformable_cards();
        ctx.select_up_to(SelectPurpose::RelicUpTo { relic: me }, options, CARDS);
        Ok(())
    }

    /// `CardCmd.Transform(transformations, ..)` in pick order ([`RunState::transform_many`]).
    fn on_cards_selected(&self, s: &mut RunState, _me: usize, cards: &[usize]) {
        let transformations: Vec<(usize, RunCard)> =
            cards.iter().map(|&i| (i, maul_from(s.deck[i]))).collect();
        s.transform_many(&transformations);
    }
}

#[sts2_macros::hooks]
impl Hooks for Claws {
    type Me = RelicId;
}
