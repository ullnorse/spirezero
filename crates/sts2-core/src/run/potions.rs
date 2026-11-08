//! Run-level potions: the out-of-combat potion pool in the game's order, `PotionFactory`'s
//! random potions, and the bridge to the combat engine's ported potions.

use super::odds::{PotionRarity, roll_potion_rarity};
use crate::content::characters::Character;
use crate::content::potions::PotionKind;
use crate::rng::Rng;

use PotionRarity::{Common, Rare, Uncommon};

/// `SharedPotionPool.GenerateAllPotions()` in declaration order (every epoch revealed), with each
/// potion's `Rarity`.
pub const SHARED_POTION_POOL: [(&str, PotionRarity); 45] = [
    ("ATTACK_POTION", Common),
    ("BEETLE_JUICE", Rare),
    ("BLESSING_OF_THE_FORGE", Uncommon),
    ("BLOCK_POTION", Common),
    ("BOTTLED_POTENTIAL", Rare),
    ("CLARITY", Uncommon),
    ("COLORLESS_POTION", Common),
    ("CURE_ALL", Uncommon),
    ("DEXTERITY_POTION", Common),
    ("DISTILLED_CHAOS", Rare),
    ("DROPLET_OF_PRECOGNITION", Rare),
    ("DUPLICATOR", Uncommon),
    ("ENERGY_POTION", Common),
    ("ENTROPIC_BREW", Rare),
    ("EXPLOSIVE_AMPOULE", Common),
    ("FAIRY_IN_A_BOTTLE", Rare),
    ("FIRE_POTION", Common),
    ("FLEX_POTION", Common),
    ("FORTIFIER", Uncommon),
    ("FRUIT_JUICE", Rare),
    ("FYSH_OIL", Uncommon),
    ("GAMBLERS_BREW", Uncommon),
    ("GIGANTIFICATION_POTION", Rare),
    ("HEART_OF_IRON", Uncommon),
    ("LIQUID_BRONZE", Uncommon),
    ("LIQUID_MEMORIES", Rare),
    ("LUCKY_TONIC", Rare),
    ("MAZALETHS_GIFT", Rare),
    ("OROBIC_ACID", Rare),
    ("POTION_OF_BINDING", Uncommon),
    ("POWDERED_DEMISE", Uncommon),
    ("POWER_POTION", Common),
    ("RADIANT_TINCTURE", Uncommon),
    ("REGEN_POTION", Uncommon),
    ("SHACKLING_POTION", Rare),
    ("SHIP_IN_A_BOTTLE", Rare),
    ("SKILL_POTION", Common),
    ("SNECKO_OIL", Rare),
    ("SPEED_POTION", Common),
    ("STABLE_SERUM", Uncommon),
    ("STRENGTH_POTION", Common),
    ("SWIFT_POTION", Common),
    ("TOUCH_OF_INSANITY", Uncommon),
    ("VULNERABLE_POTION", Common),
    ("WEAK_POTION", Common),
];

/// `PotionFactory.GetPotionOptions` for a `character`'s player with every epoch revealed.
///
/// The character's pool (`<Character>4Epoch.Potions`) then `SharedPotionPool`, in declaration
/// order, with each potion's `Rarity`.
#[must_use]
pub fn potion_options(character: Character) -> Vec<(&'static str, PotionRarity)> {
    character
        .potion_pool()
        .iter()
        .chain(&SHARED_POTION_POOL)
        .copied()
        .collect()
}

/// `PotionModel.Rarity` of a character or shared pool potion.
#[must_use]
pub fn potion_rarity(id: &str) -> Option<PotionRarity> {
    Character::ALL
        .iter()
        .flat_map(|c| c.potion_pool())
        .chain(&SHARED_POTION_POOL)
        .find(|(p, _)| *p == id)
        .map(|&(_, r)| r)
}

/// The options with `CanBeGeneratedInCombat` false: `FairyInABottle`, `FruitJuice`, `RegenPotion`
/// (`Models.Potions/{FairyInABottle,FruitJuice,RegenPotion}.cs`).
pub(crate) const NOT_GENERATED_IN_COMBAT: [&str; 3] =
    ["FAIRY_IN_A_BOTTLE", "FRUIT_JUICE", "REGEN_POTION"];

/// `PotionFactory.CreateRandomPotionsOutOfCombat(player, count, rng)`.
///
/// `Factories/PotionFactory.cs:45-90`: per potion a rarity `NextFloat`, then `NextItem` among
/// the remaining options of that rarity; a picked potion is removed from the options.
///
/// # Panics
/// Never: every rarity has more potions than a single call can take.
pub fn random_potions(character: Character, count: usize, rng: &mut Rng) -> Vec<&'static str> {
    pick_potions(potion_options(character), count, rng)
}

/// `PotionFactory.CreateRandomPotionInCombat(player, rng)` (`PotionFactory.cs:64-74`): one
/// potion of the options that can be generated in combat.
///
/// # Panics
/// Never: every rarity keeps more than one potion.
pub fn random_potion_in_combat(character: Character, rng: &mut Rng) -> &'static str {
    let options = potion_options(character)
        .into_iter()
        .filter(|(id, _)| !NOT_GENERATED_IN_COMBAT.contains(id))
        .collect();
    pick_potions(options, 1, rng)[0]
}

/// `PotionFactory.CreateRandomPotions` (`:76-89`).
fn pick_potions(
    mut options: Vec<(&'static str, PotionRarity)>,
    count: usize,
    rng: &mut Rng,
) -> Vec<&'static str> {
    let mut picked = Vec::with_capacity(count);
    for _ in 0..count {
        let rarity = roll_potion_rarity(rng);
        let of_rarity: Vec<&'static str> = options
            .iter()
            .filter(|(_, r)| *r == rarity)
            .map(|&(id, _)| id)
            .collect();
        let id = *rng.next_item(&of_rarity).expect("every rarity has potions");
        options.retain(|&(o, _)| o != id);
        picked.push(id);
    }
    picked
}

/// The combat engine's potion for `id`, if it is ported.
#[must_use]
pub fn combat_potion(id: &str) -> Option<PotionKind> {
    PotionKind::from_id(id)
}

/// The id of a combat engine potion.
#[must_use]
pub const fn potion_id(kind: PotionKind) -> &'static str {
    kind.id()
}

/// Whether a potion acts in combat without being used.
///
/// Of the pool only `FairyInABottle` does
/// (`ShouldDie`, `AfterPreventingDeath`); every other potion only has `OnUse`, so an unported one
/// can sit in the belt through a fight as long as nobody uses it.
#[must_use]
pub fn is_passive_in_combat(id: &str) -> bool {
    id == "FAIRY_IN_A_BOTTLE"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_has_every_rarity_and_the_ported_potions() {
        let necrobinder = potion_options(Character::Necrobinder);
        assert_eq!(necrobinder.len(), 48);
        for rarity in [Common, Uncommon, Rare] {
            assert!(necrobinder.iter().any(|(_, r)| *r == rarity));
        }
        // Character potions are only in their own character's pool.
        let options: Vec<_> = Character::ALL
            .into_iter()
            .flat_map(potion_options)
            .collect();
        // PotionShapedRock is a token (`TokenPotionPool`); Ambergris, Glowwater and Foul are
        // Event potions (`EventPotionPool`), not in the reward pool.
        for kind in PotionKind::ALL.into_iter().filter(|&k| {
            !matches!(
                k,
                PotionKind::PotionShapedRock
                    | PotionKind::Ambergris
                    | PotionKind::GlowwaterPotion
                    | PotionKind::FoulPotion
            )
        }) {
            assert!(
                options
                    .iter()
                    .any(|(id, _)| combat_potion(id) == Some(kind))
            );
        }
    }
}
