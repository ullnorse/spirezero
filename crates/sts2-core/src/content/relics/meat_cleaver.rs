//! `MeatCleaver` (`Models.Relics/MeatCleaver.cs`): Ancient (Tanx); `TryModifyRestSiteOptions`
//! adds `CookRestSiteOption` (`COOK`, enabled with 2+ removable deck cards: remove 2 chosen
//! cards, then +5 max HP; the run's [`RestOptionKind::Cook`]).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RestOption, RestOptionKind, RunState};

/// `CookRestSiteOption._cardsToRemove`: `IsEnabled` needs this many removable deck cards.
const CARDS: usize = 2;

pub struct MeatCleaver;

impl RelicModel for MeatCleaver {
    fn name(&self) -> &'static str {
        "Meat Cleaver"
    }

    fn try_modify_rest_site_options(
        &self,
        s: &RunState,
        _me: usize,
        options: &mut Vec<RestOption>,
    ) -> bool {
        options.push(RestOption {
            kind: RestOptionKind::Cook,
            enabled: s.removable_cards().len() >= CARDS,
        });
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for MeatCleaver {
    type Me = RelicId;
}
