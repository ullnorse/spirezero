//! `CursedRun` (`Models.Modifiers/CursedRun.cs`): `AfterActEntered` adds a random curse to the
//! deck, on `Rng.Niche` among the `CurseCardPool` cards modifiers may generate (the same list as
//! `NeowsBones`'s), at the start of every act.

use crate::content::relics::neows_bones::AVAILABLE_CURSES;
use crate::rng::RunRngType;
use crate::run::{RunCard, RunState};

/// `AfterActEntered`.
pub(super) fn after_act_entered(s: &mut RunState) {
    let id = *s.rng[RunRngType::Niche]
        .next_item(&AVAILABLE_CURSES)
        .expect("a curse to add");
    s.add_card(RunCard::new(id));
}
