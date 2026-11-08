//! `Neow` (`Models.Events/Neow.cs`): Act 1's ancient. Three relic options: two shuffled positive
//! ones, then a curse. Only options whose relic effects are ported can be chosen.

use crate::content::ancients::{AncientModel, AncientOption};
use crate::rng::Rng;
use crate::run::RunState;

/// Act 1's ancient (Overgrowth and Underdocks).
pub struct Neow;

impl AncientModel for Neow {
    fn generate_options(&self, _s: &mut RunState, rng: &mut Rng) -> Vec<AncientOption> {
        generate_options(rng)
            .into_iter()
            .map(AncientOption::relic)
            .collect()
    }
}

/// `Neow.CurseOptions`, in declaration order.
const CURSE_OPTIONS: [&str; 10] = [
    "CURSED_PEARL",
    "DOWSING_ROD",
    "HEFTY_TABLET",
    "LARGE_CAPSULE",
    "LEAFY_POULTICE",
    "NEOWS_BONES",
    "NEOWS_SACRIFICE",
    "PRECARIOUS_SHEARS",
    "SILKEN_TRESS",
    "SILVER_CRUCIBLE",
];

/// `Neow.PositiveOptions`, in declaration order.
const POSITIVE_OPTIONS: [&str; 14] = [
    "ARCANE_SCROLL",
    "BOOMING_CONCH",
    "FISHING_ROD",
    "GOLDEN_PEARL",
    "KALEIDOSCOPE",
    "LEAD_PAPERWEIGHT",
    "LOST_COFFER",
    "MASSIVE_SCROLL",
    "NEOWS_TORMENT",
    "NEW_LEAF",
    "PHIAL_HOLSTER",
    "PRECISE_SCISSORS",
    "SCROLL_BOXES",
    "WINGED_BOOTS",
];

/// The options that `Neow.GenerateInitialOptions` can append besides the two lists.
#[cfg(test)]
const APPENDED_OPTIONS: [&str; 6] = [
    "LAVA_ROCK",
    "SMALL_CAPSULE",
    "NUTRITIOUS_OYSTER",
    "STONE_HUMIDIFIER",
    "NEOWS_TALISMAN",
    "POMANDER",
];

/// Every relic Neow can offer in single player: the curses, the allowed positives and the
/// appended options.
#[cfg(test)]
fn all_options() -> impl Iterator<Item = &'static str> {
    CURSE_OPTIONS
        .into_iter()
        .chain(POSITIVE_OPTIONS)
        .chain(APPENDED_OPTIONS)
        .filter(|id| is_allowed_at_neow(id))
}

/// `Neow.AllPossibleOptions` (`Neow.cs:46-61`) as relic ids: the curse options, the positive ones,
/// then the six appended ones, in the game's order (`NeowsBones` picks from this list).
pub fn all_possible_options() -> impl Iterator<Item = &'static str> {
    CURSE_OPTIONS.into_iter().chain(POSITIVE_OPTIONS).chain([
        "LAVA_ROCK",
        "NEOWS_TALISMAN",
        "NUTRITIOUS_OYSTER",
        "POMANDER",
        "SMALL_CAPSULE",
        "STONE_HUMIDIFIER",
    ])
}

/// Whether `id`'s relic `IsAllowedAtNeow` in single player.
#[must_use]
pub fn is_allowed(id: &str) -> bool {
    is_allowed_at_neow(id)
}

/// `IsAllowedAtNeow` in single player with every epoch revealed: only `MassiveScroll`
/// (`IsAllowed` = multiplayer) is refused. `Kaleidoscope` needs every character card pool and
/// `ScrollBoxes` needs bundles, both of which hold for the all-unlocked profile.
fn is_allowed_at_neow(id: &str) -> bool {
    id != "MASSIVE_SCROLL"
}

/// `Neow.GenerateInitialOptions` (`Neow.cs:217-286`) with no modifiers, on the event `rng`: two
/// shuffled positive options, then the curse. Returns the relic ids in display order.
fn generate_options(rng: &mut Rng) -> Vec<&'static str> {
    let curses: Vec<&'static str> = CURSE_OPTIONS
        .into_iter()
        .filter(|id| is_allowed_at_neow(id))
        .collect();
    let curse = *rng.next_item(&curses).expect("Neow has curse options");
    let mut positives = POSITIVE_OPTIONS.to_vec();
    let removed: &[&str] = match curse {
        "CURSED_PEARL" => &["GOLDEN_PEARL"],
        "HEFTY_TABLET" => &["ARCANE_SCROLL"],
        "LEAFY_POULTICE" => &["NEW_LEAF"],
        "PRECARIOUS_SHEARS" => &["PRECISE_SCISSORS"],
        "NEOWS_SACRIFICE" => &["PHIAL_HOLSTER", "LOST_COFFER"],
        _ => &[],
    };
    positives.retain(|id| !removed.contains(id));
    if curse != "LARGE_CAPSULE" {
        positives.push(if rng.next_bool() {
            "LAVA_ROCK"
        } else {
            "SMALL_CAPSULE"
        });
    }
    positives.push(if rng.next_bool() {
        "NUTRITIOUS_OYSTER"
    } else {
        "STONE_HUMIDIFIER"
    });
    positives.push(if rng.next_bool() {
        "NEOWS_TALISMAN"
    } else {
        "POMANDER"
    });
    positives.retain(|id| is_allowed_at_neow(id));
    crate::shuffle::unstable_shuffle(&mut positives, rng);
    let mut options: Vec<&'static str> = positives.into_iter().take(2).collect();
    options.push(curse);
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ported_option_count() {
        let all: Vec<&str> = all_options().collect();
        let ported = all
            .iter()
            .filter(|&&id| AncientOption::relic(id).is_supported())
            .count();
        assert_eq!((ported, all.len()), (29, 29));
    }
}
