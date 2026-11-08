//! `NeowsBones` (`Models.Relics/NeowsBones.cs`): `AfterObtained` shuffles Neow's other relic
//! options (`Neow.AllPossibleOptions` order, `PlayerRng.Rewards.Shuffle`), offers 2 of them as
//! relic rewards (skipping disallowed), and after that screen adds 1 random curse (`Rng.Niche`,
//! among the `CurseCardPool` cards modifiers may generate, ordered by id) to the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::ancients::neow;
use crate::content::relics::RelicModel;
use crate::rng::{PlayerRngType, RunRngType};
use crate::run::{Reward, RunCard, RunCtx, RunError, RunState};
use crate::shuffle::unstable_shuffle;

/// `DynamicVar("Relics", 2m)`.
const RELICS: usize = 2;
/// `DynamicVar("Curses", 1m)`.
const CURSES: usize = 1;

/// `CurseCardPool` cards with `CanBeGeneratedByModifiers`, ordered by id.
pub const AVAILABLE_CURSES: [&str; 10] = [
    "CLUMSY",
    "DEBT",
    "DECAY",
    "DOUBT",
    "GUILTY",
    "INJURY",
    "NORMALITY",
    "REGRET",
    "SHAME",
    "WRITHE",
];

pub struct NeowsBones;

/// The curses part of `AfterObtained`: per curse a `NextItem` of the remaining list on
/// `Rng.Niche`, created and added to the deck.
pub fn add_curses(s: &mut RunState, count: usize) {
    let mut available = AVAILABLE_CURSES.to_vec();
    for _ in 0..count {
        let id = *s.rng[RunRngType::Niche]
            .next_item(&available)
            .expect("a curse to add");
        available.retain(|&c| c != id);
        s.add_card(RunCard::new(id));
    }
}

impl RelicModel for NeowsBones {
    fn name(&self) -> &'static str {
        "Neow's Bones"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let mut options: Vec<&'static str> = neow::all_possible_options()
            .filter(|&id| id != "NEOWS_BONES" && neow::is_allowed(id))
            .collect();
        unstable_shuffle(
            &mut options,
            &mut ctx.state().player_rng[PlayerRngType::Rewards],
        );
        let rewards: Vec<Reward> = options
            .into_iter()
            .take(RELICS)
            .map(Reward::Relic)
            .collect();
        if !rewards.is_empty() {
            ctx.offer_rewards(rewards);
        }
        ctx.then_add_curses(CURSES);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for NeowsBones {
    type Me = RelicId;
}
