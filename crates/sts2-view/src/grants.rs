//! What an event option grants, for the row's `i` (WEB72).
//!
//! The game gives an `EventOption` its hover tips (`new EventOption(this, Ornate, key,
//! HoverTipFactory.FromCardWithCardHoverTips<Clumsy>())`): the card, relic, potion or
//! enchantment the option puts in the run. The port's `EventOption` has no such field (its
//! effect is code in `choose`), so the list is kept here, read off the decompiled events, and
//! nothing in `sts2-core` (or what the RL observation encodes) changes. Only fixed grants are
//! listed: an option that grants "a random relic" has none to name.

use sts2_core::content::relics::RelicKind;
use sts2_core::run::EventOption;

use crate::model::InfoRef;

#[derive(Clone, Copy)]
enum Grant {
    Card,
    Relic,
    Potion,
    /// `ID` or `ID:amount`.
    Enchantment,
}

/// `(option TextKey, what it grants, id)`.
const TABLE: &[(&str, Grant, &str)] = &[
    (
        "AMALGAMATOR.pages.INITIAL.options.COMBINE_STRIKES",
        Grant::Card,
        "ULTIMATE_STRIKE",
    ),
    (
        "AMALGAMATOR.pages.INITIAL.options.COMBINE_DEFENDS",
        Grant::Card,
        "ULTIMATE_DEFEND",
    ),
    (
        "BUGSLAYER.pages.INITIAL.options.EXTERMINATION",
        Grant::Card,
        "EXTERMINATE",
    ),
    (
        "BUGSLAYER.pages.INITIAL.options.SQUASH",
        Grant::Card,
        "SQUASH",
    ),
    (
        "BYRDONIS_NEST.pages.INITIAL.options.TAKE",
        Grant::Card,
        "BYRDONIS_EGG",
    ),
    (
        "COLOSSAL_FLOWER.pages.REACH_DEEPER_2.options.POLLINOUS_CORE",
        Grant::Relic,
        "POLLINOUS_CORE",
    ),
    (
        "CRYSTAL_SPHERE.pages.INITIAL.options.PAYMENT_PLAN",
        Grant::Card,
        "DEBT",
    ),
    (
        "DROWNING_BEACON.pages.INITIAL.options.CLIMB",
        Grant::Relic,
        "FRESNEL_LENS",
    ),
    (
        "FIELD_OF_MAN_SIZED_HOLES.pages.INITIAL.options.RESIST",
        Grant::Card,
        "NORMALITY",
    ),
    (
        "FIELD_OF_MAN_SIZED_HOLES.pages.INITIAL.options.ENTER_YOUR_HOLE",
        Grant::Enchantment,
        "PERFECT_FIT",
    ),
    (
        "GRAVE_OF_THE_FORGOTTEN.pages.INITIAL.options.CONFRONT",
        Grant::Enchantment,
        "SOULS_POWER",
    ),
    (
        "GRAVE_OF_THE_FORGOTTEN.pages.INITIAL.options.CONFRONT",
        Grant::Card,
        "DECAY",
    ),
    (
        "GRAVE_OF_THE_FORGOTTEN.pages.INITIAL.options.ACCEPT",
        Grant::Relic,
        "FORGOTTEN_SOUL",
    ),
    (
        "LUMINOUS_CHOIR.pages.INITIAL.options.REACH_INTO_THE_FLESH",
        Grant::Card,
        "SPORE_MIND",
    ),
    (
        "POTION_COURIER.pages.INITIAL.options.GRAB_POTIONS",
        Grant::Potion,
        "FOUL_POTION",
    ),
    ("PUNCH_OFF.pages.INITIAL.options.NAB", Grant::Card, "INJURY"),
    (
        "REFLECTIONS.pages.INITIAL.options.SHATTER",
        Grant::Card,
        "BAD_LUCK",
    ),
    (
        "ROOM_FULL_OF_CHEESE.pages.INITIAL.options.SEARCH",
        Grant::Relic,
        "CHOSEN_CHEESE",
    ),
    (
        "ROUND_TEA_PARTY.pages.INITIAL.options.ENJOY_TEA",
        Grant::Relic,
        "ROYAL_POISON",
    ),
    (
        "SAPPHIRE_SEED.pages.INITIAL.options.PLANT",
        Grant::Enchantment,
        "SOWN",
    ),
    (
        "SELF_HELP_BOOK.pages.INITIAL.options.READ_THE_BACK",
        Grant::Enchantment,
        "SHARP:2",
    ),
    (
        "SELF_HELP_BOOK.pages.INITIAL.options.READ_PASSAGE",
        Grant::Enchantment,
        "NIMBLE:2",
    ),
    (
        "SELF_HELP_BOOK.pages.INITIAL.options.READ_ENTIRE_BOOK",
        Grant::Enchantment,
        "SWIFT:2",
    ),
    (
        "SPIRALING_WHIRLPOOL.pages.INITIAL.options.OBSERVE",
        Grant::Enchantment,
        "SPIRAL",
    ),
    (
        "SPIRIT_GRAFTER.pages.INITIAL.options.LET_IT_IN",
        Grant::Card,
        "METAMORPHOSIS",
    ),
    (
        "STONE_OF_ALL_TIME.pages.INITIAL.options.PUSH",
        Grant::Enchantment,
        "VIGOROUS:8",
    ),
    (
        "SUNKEN_STATUE.pages.INITIAL.options.GRAB_SWORD",
        Grant::Relic,
        "SWORD_OF_STONE",
    ),
    (
        "SUNKEN_TREASURY.pages.INITIAL.options.SECOND_CHEST",
        Grant::Card,
        "GREED",
    ),
    (
        "SYMBIOTE.pages.INITIAL.options.APPROACH",
        Grant::Enchantment,
        "CORRUPTED",
    ),
    (
        "THE_LEGENDS_WERE_TRUE.pages.INITIAL.options.NAB_THE_MAP",
        Grant::Card,
        "SPOILS_MAP",
    ),
    (
        "THIS_OR_THAT.pages.INITIAL.options.ORNATE",
        Grant::Card,
        "CLUMSY",
    ),
    ("TRIAL.pages.MERCHANT.options.GUILTY", Grant::Card, "REGRET"),
    (
        "TRIAL.pages.MERCHANT.options.INNOCENT",
        Grant::Card,
        "SHAME",
    ),
    ("TRIAL.pages.NOBLE.options.INNOCENT", Grant::Card, "REGRET"),
    (
        "TRIAL.pages.NONDESCRIPT.options.GUILTY",
        Grant::Card,
        "DOUBT",
    ),
    (
        "TRIAL.pages.NONDESCRIPT.options.INNOCENT",
        Grant::Card,
        "DOUBT",
    ),
    (
        "UNREST_SITE.pages.INITIAL.options.REST",
        Grant::Card,
        "POOR_SLEEP",
    ),
    (
        "WAR_HISTORIAN_REPY.pages.INITIAL.options.UNLOCK_CAGE",
        Grant::Relic,
        "HISTORY_COURSE",
    ),
    (
        "WAR_HISTORIAN_REPY.pages.INITIAL.options.UNLOCK_CAGE",
        Grant::Card,
        "LANTERN_KEY",
    ),
    (
        "WAR_HISTORIAN_REPY.pages.INITIAL.options.UNLOCK_CHEST",
        Grant::Card,
        "LANTERN_KEY",
    ),
    (
        "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.TENTACLE_QUILL",
        Grant::Enchantment,
        "STEADY",
    ),
    (
        "WATERLOGGED_SCRIPTORIUM.pages.INITIAL.options.PRICKLY_SPONGE",
        Grant::Enchantment,
        "STEADY",
    ),
    (
        "WELLSPRING.pages.INITIAL.options.BATHE",
        Grant::Card,
        "GUILTY",
    ),
    (
        "WOOD_CARVINGS.pages.INITIAL.options.SNAKE",
        Grant::Enchantment,
        "SLITHER",
    ),
    (
        "WOOD_CARVINGS.pages.INITIAL.options.BIRD",
        Grant::Card,
        "PECK",
    ),
    (
        "WOOD_CARVINGS.pages.INITIAL.options.TORUS",
        Grant::Card,
        "TORIC_TOUGHNESS",
    ),
    (
        "ZEN_WEAVER.pages.INITIAL.options.BREATHING_TECHNIQUES",
        Grant::Card,
        "ENLIGHTENMENT",
    ),
    (
        "DROWNING_BEACON.pages.INITIAL.options.BOTTLE",
        Grant::Potion,
        "GLOWWATER_POTION",
    ),
];

/// The doll room's options are the relics themselves (`OptionFromChoice`: the `TextKey` is the
/// relic's title).
const DOLLS: [RelicKind; 3] = [
    RelicKind::DaughterOfTheWind,
    RelicKind::MrStruggles,
    RelicKind::BingBong,
];

/// What choosing `option` adds to the run, as far as the game names it.
#[must_use]
pub fn of(option: &EventOption) -> Vec<InfoRef> {
    let key = option.text_key.as_str();
    if let Some(doll) = DOLLS.iter().find(|d| d.name() == key) {
        return vec![InfoRef::Relic(doll.id().to_owned())];
    }
    TABLE
        .iter()
        .filter(|(k, ..)| *k == key)
        .map(|&(_, grant, id)| match grant {
            Grant::Card => InfoRef::Card(id.to_owned()),
            Grant::Relic => InfoRef::Relic(id.to_owned()),
            Grant::Potion => InfoRef::Potion(id.to_owned()),
            Grant::Enchantment => InfoRef::Enchantment(id.to_owned()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_core::content::cards::CardKind;
    use sts2_core::content::enchantments::EnchantmentKind;
    use sts2_core::content::potions::PotionKind;

    /// Every id in the table is a model the engine has.
    #[test]
    fn every_id_resolves() {
        for &(key, grant, id) in TABLE {
            let ok = match grant {
                Grant::Card => CardKind::from_id(id).is_some(),
                Grant::Relic => RelicKind::from_id(id).is_some(),
                Grant::Potion => PotionKind::from_id(id).is_some(),
                Grant::Enchantment => {
                    EnchantmentKind::from_id(id.split(':').next().unwrap_or(id)).is_some()
                }
            };
            assert!(ok, "{key}: unknown id {id}");
        }
    }

    #[test]
    fn this_or_that_names_clumsy() {
        let option = EventOption::new("THIS_OR_THAT.pages.INITIAL.options.ORNATE", "ORNATE");
        assert_eq!(of(&option), vec![InfoRef::Card("CLUMSY".to_owned())]);
    }

    #[test]
    fn a_doll_is_its_relic() {
        let option = EventOption::with_arg(RelicKind::BingBong.name(), "TAKE", 2);
        assert_eq!(of(&option), vec![InfoRef::Relic("BING_BONG".to_owned())]);
    }
}
