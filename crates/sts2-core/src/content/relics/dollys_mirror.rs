//! `DollysMirror` (`Models.Relics/DollysMirror.cs`): `AfterObtained` lets the player pick one
//! deck card (`FromDeckGeneric`, not a Quest) and adds a copy of it to the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::cards::CardType;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

pub struct DollysMirror;

impl RelicModel for DollysMirror {
    fn name(&self) -> &'static str {
        "Dolly's Mirror"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let options: Vec<usize> = (0..s.deck.len())
            .filter(|&i| s.deck[i].card_type() != Some(CardType::Quest))
            .collect();
        ctx.select(SelectPurpose::Clone, options, 1)
    }
}

#[sts2_macros::hooks]
impl Hooks for DollysMirror {
    type Me = RelicId;
}
