//! Characters (`Models.Characters/*.cs`, `CharacterModel`): starting stats, starter deck and
//! relic, and the card, relic and potion pools.
//!
//! Every character's `Player` holds every resource (orbs, stars, pets); a character only decides
//! the starting values and the pools (`docs/decisions.md`, 2026-10-17).

mod defect;
mod ironclad;
mod necrobinder;
mod regent;
mod silent;

use std::sync::OnceLock;

use crate::content::IdTable;
use crate::content::relics::RelicKind;
use crate::run::cards::PoolCard;
use crate::run::odds::PotionRarity;
use crate::run::relic_bag::RelicRarity;

/// A playable character (`CharacterModel`; `Deprived` and `RandomCharacter` are not playable).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Character {
    /// `Character.Ironclad`.
    Ironclad,
    /// `Character.Silent`.
    Silent,
    /// `Character.Defect`.
    Defect,
    /// `Character.Regent`.
    Regent,
    /// `Character.Necrobinder`.
    #[default]
    Necrobinder,
}

impl Character {
    /// Every character, in the game's character-select order.
    pub const ALL: [Self; 5] = [
        Self::Ironclad,
        Self::Silent,
        Self::Defect,
        Self::Regent,
        Self::Necrobinder,
    ];

    /// `ModelDb.AllCharacters` (`ModelDb.cs:145-152`), which `UnlockState.Characters` keeps (every
    /// epoch revealed): the order of `CharacterCardPools` and of `Orobas`'s character roll.
    pub const MODEL_DB_ORDER: [Self; 5] = [
        Self::Ironclad,
        Self::Silent,
        Self::Regent,
        Self::Necrobinder,
        Self::Defect,
    ];

    /// The class name (`Ironclad`), as the referee's `start_run` takes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ironclad => "Ironclad",
            Self::Silent => "Silent",
            Self::Defect => "Defect",
            Self::Regent => "Regent",
            Self::Necrobinder => "Necrobinder",
        }
    }

    /// The character with class name `name` (case-insensitive).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|c| c.name().eq_ignore_ascii_case(name))
    }

    /// `StartingHp`.
    #[must_use]
    pub const fn starting_hp(self) -> i32 {
        match self {
            Self::Ironclad => 80,
            Self::Silent => 70,
            Self::Defect | Self::Regent => 75,
            Self::Necrobinder => 66,
        }
    }

    /// `StartingGold`: 99 for every character.
    #[must_use]
    pub const fn starting_gold(self) -> i32 {
        99
    }

    /// `MaxEnergy`: the `CharacterModel` default 3; no character overrides it.
    #[must_use]
    pub const fn max_energy(self) -> i32 {
        3
    }

    /// `BaseOrbSlotCount`: the default 0, Defect 3 (`Defect.cs:63`). Every combat's orb queue
    /// starts with this capacity (`PlayerCombatState.cs:137-139`).
    #[must_use]
    pub const fn base_orb_slots(self) -> u8 {
        match self {
            Self::Defect => 3,
            _ => 0,
        }
    }

    /// `StartingDeck`, in order, as card ids (`ModelId.Entry`).
    #[must_use]
    pub const fn starting_deck(self) -> &'static [&'static str] {
        match self {
            Self::Ironclad => &ironclad::STARTING_DECK,
            Self::Silent => &silent::STARTING_DECK,
            Self::Defect => &defect::STARTING_DECK,
            Self::Regent => &regent::STARTING_DECK,
            Self::Necrobinder => &necrobinder::STARTING_DECK,
        }
    }

    /// `StartingRelics` (one relic for every character).
    #[must_use]
    pub const fn starting_relic(self) -> RelicKind {
        match self {
            Self::Ironclad => RelicKind::BurningBlood,
            Self::Silent => RelicKind::RingOfTheSnake,
            Self::Defect => RelicKind::CrackedCore,
            Self::Regent => RelicKind::DivineRight,
            Self::Necrobinder => RelicKind::BoundPhylactery,
        }
    }

    /// `CardPool.GenerateAllCards()`, in declaration order.
    #[must_use]
    pub const fn card_pool(self) -> &'static [PoolCard] {
        match self {
            Self::Ironclad => &ironclad::CARD_POOL,
            Self::Silent => &silent::CARD_POOL,
            Self::Defect => &defect::CARD_POOL,
            Self::Regent => &regent::CARD_POOL,
            Self::Necrobinder => &necrobinder::CARD_POOL,
        }
    }

    /// The card pool's cards whose `CanonicalKeywords` contain `Ethereal`.
    #[must_use]
    pub const fn ethereal_cards(self) -> &'static [&'static str] {
        match self {
            Self::Ironclad => &ironclad::ETHEREAL,
            Self::Silent => &silent::ETHEREAL,
            Self::Defect => &defect::ETHEREAL,
            Self::Regent => &regent::ETHEREAL,
            Self::Necrobinder => &necrobinder::ETHEREAL,
        }
    }

    /// The card pool's cards that override `CanBeGeneratedInCombat` to false.
    #[must_use]
    pub const fn not_generated_cards(self) -> &'static [&'static str] {
        match self {
            Self::Ironclad => &ironclad::NOT_GENERATED,
            Self::Silent => &silent::NOT_GENERATED,
            Self::Defect => &defect::NOT_GENERATED,
            Self::Regent => &regent::NOT_GENERATED,
            Self::Necrobinder => &necrobinder::NOT_GENERATED,
        }
    }

    /// The card pool's cards whose `EnergyCost` is canonically 0 and not X.
    #[must_use]
    pub const fn zero_cost_cards(self) -> &'static [&'static str] {
        match self {
            Self::Ironclad => &ironclad::ZERO_COST,
            Self::Silent => &silent::ZERO_COST,
            Self::Defect => &defect::ZERO_COST,
            Self::Regent => &regent::ZERO_COST,
            Self::Necrobinder => &necrobinder::ZERO_COST,
        }
    }

    /// `RelicPool.GenerateAllRelics()`, in declaration order, with each relic's `Rarity`.
    #[must_use]
    pub const fn relic_pool(self) -> &'static [(&'static str, RelicRarity)] {
        match self {
            Self::Ironclad => &ironclad::RELIC_POOL,
            Self::Silent => &silent::RELIC_POOL,
            Self::Defect => &defect::RELIC_POOL,
            Self::Regent => &regent::RELIC_POOL,
            Self::Necrobinder => &necrobinder::RELIC_POOL,
        }
    }

    /// `PotionPool` (`<Character>4Epoch.Potions`), in declaration order, with each potion's
    /// `Rarity`.
    #[must_use]
    pub const fn potion_pool(self) -> &'static [(&'static str, PotionRarity)] {
        match self {
            Self::Ironclad => &ironclad::POTION_POOL,
            Self::Silent => &silent::POTION_POOL,
            Self::Defect => &defect::POTION_POOL,
            Self::Regent => &regent::POTION_POOL,
            Self::Necrobinder => &necrobinder::POTION_POOL,
        }
    }

    /// The character whose card pool holds `id`, if any (no card is in two pools).
    #[must_use]
    pub fn of_card(id: &str) -> Option<Self> {
        Self::pool_card(id).map(|(c, _)| c)
    }

    /// The character whose card pool holds `id` and the card's entry in it, if any: one hash
    /// lookup ([`IdTable`], built on first use) instead of a scan of every pool.
    #[must_use]
    pub fn pool_card(id: &str) -> Option<(Self, &'static PoolCard)> {
        static BY_ID: OnceLock<IdTable<(Character, &'static PoolCard)>> = OnceLock::new();
        BY_ID
            .get_or_init(|| {
                IdTable::new(
                    Self::ALL
                        .into_iter()
                        .flat_map(|c| c.card_pool().iter().map(move |card| (card.id, (c, card))))
                        .collect::<Vec<_>>()
                        .into_iter(),
                )
            })
            .get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::RunCard;
    use crate::run::odds::CardRarity;

    #[test]
    fn pool_card_lookup_matches_a_scan() {
        let scan = |id: &str| {
            Character::ALL.into_iter().find_map(|c| {
                c.card_pool()
                    .iter()
                    .find(|card| card.id == id)
                    .map(|card| (c, card))
            })
        };
        for c in Character::ALL {
            for card in c.card_pool() {
                let found = Character::pool_card(card.id).expect("a pool card");
                let expected = scan(card.id).expect("a pool card");
                assert_eq!(found.0, expected.0, "{}", card.id);
                assert!(std::ptr::eq(found.1, expected.1), "{}", card.id);
            }
        }
        for id in ["", "STRIKE", "MADNESS", "BECKON", "NOT_A_CARD"] {
            assert_eq!(
                Character::pool_card(id).map(|(c, _)| c),
                scan(id).map(|(c, _)| c)
            );
        }
    }

    #[test]
    fn pools_have_the_surveyed_shape() {
        // `docs/specs/characters.md` §3-5.
        for c in Character::ALL {
            let pool = c.card_pool();
            let expected = if c == Character::Ironclad { 90 } else { 91 };
            assert_eq!(pool.len(), expected, "{c:?}");
            assert_eq!(pool.iter().filter(|p| p.multiplayer_only).count(), 5);
            assert_eq!(c.relic_pool().len(), 8);
            assert_eq!(c.potion_pool().len(), 3);
            let starter = c.starting_relic().id();
            assert!(
                c.relic_pool()
                    .iter()
                    .any(|&(id, r)| id == starter && r == RelicRarity::Starter)
            );
            // Ported cards agree with the generated lists.
            for card in pool {
                let Some(kind) = RunCard::new(card.id).to_combat().map(|d| d.kind) else {
                    continue;
                };
                let zero = kind.cost() == 0 && !kind.has_energy_cost_x();
                assert_eq!(c.zero_cost_cards().contains(&card.id), zero, "{}", card.id);
            }
            for id in c.starting_deck() {
                let card = pool.iter().find(|p| p.id == *id).expect("a pool card");
                assert_eq!(card.rarity, CardRarity::Basic, "{id}");
                assert_eq!(Character::of_card(id), Some(c));
            }
        }
    }
}
