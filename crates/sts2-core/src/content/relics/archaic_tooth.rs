//! `ArchaicTooth` (`Models.Relics/ArchaicTooth.cs`): `AfterObtained` transforms the first deck
//! card with a transcendence upgrade (`Bash`, `Neutralize`, `Unleash`, `FallingStar`,
//! `Dualcast`) into its upgrade (`Break`, `Suppress`, `Protector`, `MeteorShower`, `Quadcast`),
//! upgraded if the original was, with a copy of its enchantment. `Orobas` offers it only when
//! `SetupForPlayer` finds such a card, which records both cards (`StarterCard`, `AncientCard`);
//! `AfterObtained` looks the card up again. `TranscendenceCards` is `DustyTome`'s blacklist.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{RunCard, RunCtx, RunError, RunState};

/// `TranscendenceUpgrades`, in declaration order.
const TRANSCENDENCE_UPGRADES: [(&str, &str); 5] = [
    ("BASH", "BREAK"),
    ("NEUTRALIZE", "SUPPRESS"),
    ("UNLEASH", "PROTECTOR"),
    ("FALLING_STAR", "METEOR_SHOWER"),
    ("DUALCAST", "QUADCAST"),
];

/// `TranscendenceCards`: the upgrades.
pub const TRANSCENDENCE_CARDS: [&str; 5] = [
    "BREAK",
    "SUPPRESS",
    "PROTECTOR",
    "METEOR_SHOWER",
    "QUADCAST",
];

pub struct ArchaicTooth;

/// `ArchaicTooth`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `StarterCard`.
    pub starter_card: Option<RunCard>,
    /// `AncientCard`.
    pub ancient_card: Option<RunCard>,
}

/// `GetTranscendenceStarterCard`: the deck index of the first card with an upgrade.
fn starter_card(s: &RunState) -> Option<usize> {
    s.deck
        .iter()
        .position(|c| TRANSCENDENCE_UPGRADES.iter().any(|&(from, _)| from == c.id))
}

/// `GetTranscendenceTransformedCard(starterCard)`: the upgrade (`Doubt` for any other card),
/// upgraded if the original is, with a copy of its enchantment.
fn transformed_card(original: RunCard) -> RunCard {
    let Some(&(_, id)) = TRANSCENDENCE_UPGRADES
        .iter()
        .find(|&&(from, _)| from == original.id)
    else {
        return RunCard::new("DOUBT");
    };
    let mut card = RunCard::new(id);
    if original.upgrade_level > 0 {
        card.upgrade();
    }
    card.enchantment = original.enchantment;
    card
}

/// `SetupForPlayer(player)`: the relic's fields, or `None` if the deck has no card to
/// transcend.
#[must_use]
pub fn setup_for_player(s: &RunState) -> Option<RelicData> {
    let starter = s.deck[starter_card(s)?];
    Some(RelicData::ArchaicTooth(Data {
        starter_card: Some(starter),
        ancient_card: Some(transformed_card(starter)),
    }))
}

impl RelicModel for ArchaicTooth {
    fn name(&self) -> &'static str {
        "Archaic Tooth"
    }

    fn init_data(&self) -> RelicData {
        RelicData::ArchaicTooth(Data::default())
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        // The game throws on a deck without one; `Orobas` only offers it with one.
        let index = starter_card(s).ok_or(RunError::UnsupportedRelic("ARCHAIC_TOOTH"))?;
        let replacement = transformed_card(s.deck[index]);
        s.transform_to(index, replacement);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for ArchaicTooth {
    type Me = RelicId;
}
