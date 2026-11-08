//! `Darv` (`Models.Events/Darv.cs`): a shared ancient (Acts 2-3). Each of 11 relic sets whose
//! filter passes contributes its relic (`NextItem` of a one-relic array, which still draws);
//! the list is `UnstableShuffle`d, then on `NextBool` the first 2 plus a `DustyTome` (set up on
//! the player's `rewards` stream), else the first 3. Every draw is on the event `Rng`.

use crate::content::ancients::{AncientModel, AncientOption};
use crate::content::relics::dusty_tome;
use crate::rng::Rng;
use crate::run::RunState;

/// Act index 1 (the second act): `CurrentActIndex == 1`.
const ACT_2: usize = 1;

/// A `ValidRelicSet`'s filter (`Darv.cs:171-200`).
#[derive(Clone, Copy)]
enum Filter {
    Always,
    /// `!Modifiers.Any(m => m.ClearsPlayerDeck)`: no modifier is modelled, so always.
    NoDeckClearingModifier,
    /// `CurrentActIndex == 1`.
    InAct2,
    /// `CurrentActIndex >= 1`.
    FromAct2,
}

impl Filter {
    const fn passes(self, s: &RunState) -> bool {
        match self {
            Self::Always | Self::NoDeckClearingModifier => true,
            Self::InAct2 => s.act_index == ACT_2,
            Self::FromAct2 => s.act_index >= ACT_2,
        }
    }
}

/// `Darv._validRelicSets`, in declaration order (each set holds one relic).
const VALID_RELIC_SETS: [(Filter, &str); 11] = [
    (Filter::Always, "ASTROLABE"),
    (Filter::Always, "BLACK_STAR"),
    (Filter::Always, "CALLING_BELL"),
    (Filter::Always, "EMPTY_CAGE"),
    (Filter::NoDeckClearingModifier, "PANDORAS_BOX"),
    (Filter::Always, "RUNIC_PYRAMID"),
    (Filter::Always, "SNECKO_EYE"),
    (Filter::InAct2, "ECTOPLASM"),
    (Filter::InAct2, "SOZU"),
    (Filter::FromAct2, "PHILOSOPHERS_STONE"),
    (Filter::FromAct2, "VELVET_CHOKER"),
];

/// Act 2 or 3's shared ancient.
pub struct Darv;

impl AncientModel for Darv {
    fn generate_options(&self, s: &mut RunState, rng: &mut Rng) -> Vec<AncientOption> {
        let mut source: Vec<AncientOption> = VALID_RELIC_SETS
            .iter()
            .filter(|(filter, _)| filter.passes(s))
            .map(|&(_, relic)| {
                AncientOption::relic(rng.next_item(&[relic]).copied().expect("one relic"))
            })
            .collect();
        crate::shuffle::unstable_shuffle(&mut source, rng);
        if rng.next_bool() {
            source.truncate(2);
            source.push(AncientOption::Relic {
                id: "DUSTY_TOME",
                data: Some(dusty_tome::setup_for_player(s)),
            });
        } else {
            source.truncate(3);
        }
        source
    }
}

#[cfg(test)]
/// `Darv.AllPossibleOptions` (`Darv.cs:54-55`) as relic ids: every set's relic, then
/// `DustyTome`.
pub fn all_possible_options() -> impl Iterator<Item = &'static str> {
    VALID_RELIC_SETS
        .into_iter()
        .map(|(_, relic)| relic)
        .chain(["DUSTY_TOME"])
}
