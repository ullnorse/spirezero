//! `TouchOfOrobas` (`Models.Relics/TouchOfOrobas.cs`): `AfterObtained` replaces the owner's
//! starter relic (`RelicCmd.Replace`) with its upgrade (`RefinementUpgrades`; `Circlet` for any
//! other starter). `Orobas` offers it only when `SetupForPlayer` finds a starter relic (the
//! first relic of `Starter` rarity), which records both (`StarterRelic`, `UpgradedRelic`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{RunCtx, RunError, RunState};

/// `RefinementUpgrades`, in declaration order.
const REFINEMENT_UPGRADES: [(&str, &str); 5] = [
    ("BURNING_BLOOD", "BLACK_BLOOD"),
    ("RING_OF_THE_SNAKE", "RING_OF_THE_DRAKE"),
    ("DIVINE_RIGHT", "DIVINE_DESTINY"),
    ("BOUND_PHYLACTERY", "PHYLACTERY_UNBOUND"),
    ("CRACKED_CORE", "INFUSED_CORE"),
];

/// The relics whose `Rarity` is `Starter`: the five starters and their upgrades.
const STARTER_RELICS: [&str; 10] = [
    "BLACK_BLOOD",
    "BOUND_PHYLACTERY",
    "BURNING_BLOOD",
    "CRACKED_CORE",
    "DIVINE_DESTINY",
    "DIVINE_RIGHT",
    "INFUSED_CORE",
    "PHYLACTERY_UNBOUND",
    "RING_OF_THE_DRAKE",
    "RING_OF_THE_SNAKE",
];

pub struct TouchOfOrobas;

/// `TouchOfOrobas`'s fields (`ModelId.Entry`s).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `StarterRelic`.
    pub starter_relic: Option<&'static str>,
    /// `UpgradedRelic`.
    pub upgraded_relic: Option<&'static str>,
}

/// `GetStarterRelic(p)`: the index of the first relic of `Starter` rarity.
fn starter_relic(s: &RunState) -> Option<usize> {
    s.relics
        .iter()
        .position(|r| STARTER_RELICS.contains(&r.kind.id()))
}

/// `GetUpgradedStarterRelic(starterRelic)`.
fn upgraded(starter: &str) -> &'static str {
    REFINEMENT_UPGRADES
        .iter()
        .find(|&&(from, _)| from == starter)
        .map_or("CIRCLET", |&(_, to)| to)
}

/// `id` as the `'static` string of an upgraded starter relic, if it is one ([`crate::text`]).
pub fn static_relic_id(id: &str) -> Option<&'static str> {
    REFINEMENT_UPGRADES
        .iter()
        .flat_map(|&pair| <[&str; 2]>::from(pair))
        .chain(["CIRCLET"])
        .find(|&known| known == id)
}

/// `SetupForPlayer(player)`: the relic's fields, or `None` without a starter relic.
#[must_use]
pub fn setup_for_player(s: &RunState) -> Option<RelicData> {
    let starter = s.relics[starter_relic(s)?].kind.id();
    Some(RelicData::TouchOfOrobas(Data {
        starter_relic: Some(starter),
        upgraded_relic: Some(upgraded(starter)),
    }))
}

impl RelicModel for TouchOfOrobas {
    fn name(&self) -> &'static str {
        "Touch of Orobas"
    }

    fn init_data(&self) -> RelicData {
        RelicData::TouchOfOrobas(Data::default())
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let RelicData::TouchOfOrobas(data) = s.relics[me].data else {
            unreachable!("a TouchOfOrobas has TouchOfOrobas data");
        };
        // `StarterRelic ?? First(Starter)` (which throws without one), then `GetRelicById`.
        let index = match data.starter_relic {
            Some(id) => s.relics.iter().position(|r| r.kind.id() == id),
            None => starter_relic(s),
        }
        .ok_or(RunError::UnsupportedRelic("TOUCH_OF_OROBAS"))?;
        let replacement = data
            .upgraded_relic
            .unwrap_or_else(|| upgraded(s.relics[index].kind.id()));
        ctx.replace_relic(index, replacement)
    }
}

#[sts2_macros::hooks]
impl Hooks for TouchOfOrobas {
    type Me = RelicId;
}
