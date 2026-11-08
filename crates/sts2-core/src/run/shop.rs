//! The merchant (`Rooms/MerchantRoom.cs`, `Entities.Merchant/*.cs`): a shop's inventory,
//! generated on entry, and its prices.
//!
//! Card rarities and upgrade checks draw on the player's `rewards` stream; item picks and prices
//! on `shops`. The price arithmetic is `f32` and lives in the odds module.

use super::ascension::AscensionLevel;
use super::cards::{PoolCard, RunCard, unlocked_cards, unlocked_colorless_cards};
use super::hooks;
use super::odds::{
    CardRarity, CardRarityOddsType, MERCHANT_UPGRADE_BASE, PotionRarity, UpgradeRoll,
    merchant_card_cost, merchant_potion_cost, merchant_relic_cost, roll_for_upgrade,
};
use super::potions::{potion_rarity, random_potions};
use super::relic_bag::{Bag, RelicRarity};
use super::rewards::{FALLBACK_RELIC, next_allowed_rarity, roll_relic_rarity};
use super::state::RunState;
use crate::content::cards::CardType;
use crate::decimal::Decimal;
use crate::rng::PlayerRngType;

/// `MerchantInventory._coloredCardTypes`.
const CHARACTER_CARD_TYPES: [CardType; 5] = [
    CardType::Attack,
    CardType::Attack,
    CardType::Skill,
    CardType::Skill,
    CardType::Power,
];

/// `MerchantInventory._colorlessCardRarities`.
const COLORLESS_CARD_RARITIES: [CardRarity; 2] = [CardRarity::Uncommon, CardRarity::Rare];

/// Relic slots: two rolled rarities, then a Shop relic.
const SHOP_RELIC_SLOTS: usize = 3;

/// `MerchantPotionEntry`s per shop.
const SHOP_POTIONS: usize = 3;

/// `MerchantCardRemovalEntry.CalcCost`: `BaseCost + PriceIncrease × CardShopRemovalsUsed`,
/// 75 + 25n, or 100 + 50n from `Inflation` on.
const fn removal_cost(ascension: u8, removals_used: i32) -> i32 {
    if AscensionLevel::Inflation.is_active(ascension) {
        100 + 50 * removals_used
    } else {
        75 + 25 * removals_used
    }
}

/// Relics whose `IsAllowedInShops` is false (`Models.Relics/*.cs`).
pub(crate) const NOT_IN_SHOPS: [&str; 5] = [
    "AMETHYST_AUBERGINE",
    "THE_COURIER",
    "LUCKY_FYSH",
    "OLD_COIN",
    "BOWLER_HAT",
];

/// What a card slot generates (`MerchantCardEntry`'s `_cardType` or `_cardRarity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardSlot {
    /// A character card of this type, rarity rolled.
    Character(CardType),
    /// A colorless card of this rarity.
    Colorless(CardRarity),
}

/// `MerchantCardEntry`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShopCard {
    /// Which slot it is.
    pub slot: CardSlot,
    /// `CreationResult.Card`; `None` once bought.
    pub card: Option<RunCard>,
    /// `IsOnSale`.
    pub on_sale: bool,
    /// `_cost`, before `ModifyMerchantPrice`.
    pub base_cost: i32,
}

/// `MerchantRelicEntry`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShopRelic {
    /// `Model`; `None` once bought.
    pub id: Option<&'static str>,
    /// `_cost`, before `ModifyMerchantPrice`.
    pub base_cost: i32,
}

/// `MerchantPotionEntry`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShopPotion {
    /// `Model`; `None` once bought.
    pub id: Option<&'static str>,
    /// `_cost`, before `ModifyMerchantPrice`.
    pub base_cost: i32,
}

/// An entry a relic buys without paying (`OnTryPurchaseWrapper(inventory, ignoreCost: true)`),
/// by index into its list of [`Shop`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShopEntry {
    /// The card at this index of [`Shop::cards`].
    Card(usize),
    /// The relic at this index of [`Shop::relics`].
    Relic(usize),
    /// The potion at this index of [`Shop::potions`].
    Potion(usize),
    /// The card removal, not cancelable.
    Removal,
}

/// `MerchantInventory.CreateForNormalMerchant` for the one player.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Shop {
    /// The character cards, then the colorless ones (`CardEntries`).
    pub cards: Vec<ShopCard>,
    /// `RelicEntries`.
    pub relics: Vec<ShopRelic>,
    /// `PotionEntries`.
    pub potions: Vec<ShopPotion>,
    /// `MerchantCardRemovalEntry._cost`.
    pub removal_base_cost: i32,
    /// `MerchantCardRemovalEntry.Used`.
    pub removal_used: bool,
    /// The entries a relic is still to buy for free, the next first (`LordsParasol`); the run
    /// buys them before the player acts in the shop.
    pub free_purchases: Vec<ShopEntry>,
}

/// `MerchantEntry.Cost` in a merchant room: `(int)ModifyMerchantPrice(_cost)`.
#[must_use]
pub fn price(s: &RunState, base_cost: i32) -> i32 {
    hooks::modify_merchant_price(s, Decimal::from(base_cost)).to_int_truncating()
}

impl Shop {
    /// `CreateForNormalMerchant` (`MerchantInventory.cs:78-148`): the sale index, 5 character
    /// cards, 2 colorless cards, 3 relics, 3 potions and the card removal, in that order.
    ///
    /// # Panics
    /// If a slot's pool is empty (the game throws).
    pub fn generate(s: &mut RunState) -> Self {
        let sale = usize::try_from(s.player_rng[PlayerRngType::Shops].next_int(5))
            .expect("NextInt is non-negative");
        let mut shop = Self {
            cards: Vec::new(),
            relics: Vec::new(),
            potions: Vec::new(),
            removal_base_cost: removal_cost(s.ascension, s.card_shop_removals_used),
            removal_used: !super::modifiers::allows_merchant_card_removal(s),
            free_purchases: Vec::new(),
        };
        for (i, card_type) in CHARACTER_CARD_TYPES.into_iter().enumerate() {
            let mut entry = shop.populate_card(s, CardSlot::Character(card_type));
            // `SetOnSale` recomputes the cost (a second draw), then halves it.
            if i == sale {
                entry.on_sale = true;
                entry.base_cost = card_cost(s, &entry) / 2;
            }
            shop.cards.push(entry);
        }
        for rarity in COLORLESS_CARD_RARITIES {
            let entry = shop.populate_card(s, CardSlot::Colorless(rarity));
            shop.cards.push(entry);
        }
        // Both rolled rarities come before any pull.
        let mut rarities = Vec::with_capacity(SHOP_RELIC_SLOTS);
        for _ in 1..SHOP_RELIC_SLOTS {
            rarities.push(roll_relic_rarity(&mut s.player_rng[PlayerRngType::Rewards]));
        }
        rarities.push(RelicRarity::Shop);
        for rarity in rarities {
            let entry = fill_relic(s, rarity, &[]);
            shop.relics.push(entry);
        }
        let potions = random_potions(
            s.character,
            SHOP_POTIONS,
            &mut s.player_rng[PlayerRngType::Shops],
        );
        for id in potions {
            shop.potions.push(potion_entry(s, id));
        }
        shop
    }

    /// `MerchantCardEntry.Populate` (`MerchantCardEntry.cs:68-99`): a card from the slot's pool
    /// minus the cards stocked now (`CardFactory.CreateForMerchant`), then its cost.
    fn populate_card(&self, s: &mut RunState, slot: CardSlot) -> ShopCard {
        let stocked: Vec<&str> = self
            .cards
            .iter()
            .filter_map(|c| c.card)
            .map(|c| c.id)
            .collect();
        let pool: Vec<&PoolCard> = match slot {
            CardSlot::Character(_) => {
                let mut pool: Vec<&PoolCard> = unlocked_cards(s.character).collect();
                if let Some(extra) = super::modifiers::merchant_extra_character(s) {
                    pool.extend(unlocked_cards(extra));
                }
                pool
            }
            CardSlot::Colorless(_) => unlocked_colorless_cards().collect(),
        };
        let pool: Vec<&PoolCard> = pool
            .into_iter()
            .filter(|c| !stocked.contains(&c.id) && c.rarity != CardRarity::Basic)
            .collect();
        let items: Vec<&PoolCard> = match slot {
            CardSlot::Character(card_type) => {
                let rolled = s.card_rarity_odds.roll_without_changing_future_odds(
                    CardRarityOddsType::Shop,
                    &mut s.player_rng[PlayerRngType::Rewards],
                );
                let rarity = next_allowed_rarity(rolled, |r| {
                    pool.iter()
                        .any(|c| c.rarity == r && c.card_type == card_type)
                })
                .expect("the shop has a card of every type");
                pool.into_iter()
                    .filter(|c| c.rarity == rarity && c.card_type == card_type)
                    .collect()
            }
            CardSlot::Colorless(rarity) => {
                pool.into_iter().filter(|c| c.rarity == rarity).collect()
            }
        };
        let picked = s.player_rng[PlayerRngType::Shops]
            .next_item(&items)
            .expect("a card to stock");
        let mut card = [RunCard::new(picked.id)];
        // `RollForUpgrade(card, -999999999m)` on `Rewards`: the draw is made, the card never
        // upgrades.
        let roll = UpgradeRoll {
            base_chance: MERCHANT_UPGRADE_BASE,
            act_index: s.act_index,
            ascension: s.ascension,
            rarity: picked.rarity,
            upgradable: card[0].is_upgradable(),
        };
        if roll_for_upgrade(roll, &mut s.player_rng[PlayerRngType::Rewards]) {
            card[0].upgrade_level += 1;
        }
        // `Hook.ModifyMerchantCardCreationResults` (the eggs upgrade the card), then the cost.
        hooks::modify_merchant_card_creation_results(s, &mut card);
        let mut entry = ShopCard {
            slot,
            card: Some(card[0]),
            on_sale: false,
            base_cost: 0,
        };
        entry.base_cost = card_cost(s, &entry);
        entry
    }

    /// `MerchantInventory.UpdateEntries` after a purchase: every stocked card goes through
    /// `ModifyMerchantCardCreationResults` again (a relic just bought may upgrade it).
    pub(super) fn update_entries(&mut self, s: &RunState) {
        for entry in &mut self.cards {
            if let Some(card) = &mut entry.card {
                hooks::modify_merchant_card_creation_results(s, std::slice::from_mut(card));
            }
        }
    }

    /// `RestockAfterPurchase` for card slot `i` (`TheCourier`): off sale, populated again while
    /// the bought card still counts as stocked.
    pub(super) fn restock_card(&mut self, s: &mut RunState, i: usize) {
        let entry = self.populate_card(s, self.cards[i].slot);
        self.cards[i] = entry;
    }

    /// `RestockAfterPurchase` for relic slot `i`: a rarity on `rewards`, then the back of the
    /// bag minus the relics on the shelf (the bought one included).
    pub(super) fn restock_relic(&mut self, s: &mut RunState, i: usize) {
        let rarity = roll_relic_rarity(&mut s.player_rng[PlayerRngType::Rewards]);
        let on_shelf: Vec<&'static str> = self.relics.iter().filter_map(|r| r.id).collect();
        self.relics[i] = fill_relic(s, rarity, &on_shelf);
    }

    /// `RestockAfterPurchase` for potion slot `i`: one random potion on `shops`, no blacklist.
    pub(super) fn restock_potion(&mut self, s: &mut RunState, i: usize) {
        let id = random_potions(s.character, 1, &mut s.player_rng[PlayerRngType::Shops])[0];
        self.potions[i] = potion_entry(s, id);
    }
}

/// `MerchantCardEntry.CalcCost` without the sale halving: base 50/75/150 by rarity.
fn card_cost(s: &mut RunState, entry: &ShopCard) -> i32 {
    let rarity = match entry.slot {
        CardSlot::Colorless(rarity) => rarity,
        CardSlot::Character(_) => {
            entry
                .card
                .and_then(RunCard::info)
                .expect("a stocked character card")
                .rarity
        }
    };
    let base = match rarity {
        CardRarity::Rare => 150,
        CardRarity::Uncommon => 75,
        _ => 50,
    };
    let colorless = matches!(entry.slot, CardSlot::Colorless(_));
    merchant_card_cost(base, colorless, &mut s.player_rng[PlayerRngType::Shops])
}

/// `MerchantRelicEntry.FillSlot` (`MerchantRelicEntry.cs:37-45`, `RelicFactory.cs:73-78`): the
/// back of the player's bag among relics allowed in shops and not in `blacklist` (`Circlet` if
/// none), removed from the shared bag too, then its cost.
fn fill_relic(s: &mut RunState, rarity: RelicRarity, blacklist: &[&str]) -> ShopRelic {
    let pulled = s.with_relic_bag(Bag::Player, |bag, s| {
        bag.pull_from_back(rarity, s, |id| {
            !NOT_IN_SHOPS.contains(&id) && !blacklist.contains(&id)
        })
    });
    let (id, merchant_cost) = match pulled {
        Some((id, rarity)) => (id, merchant_cost(rarity)),
        // `Circlet` is `RelicRarity.None`: `MerchantCost` 1.
        None => (FALLBACK_RELIC, 1),
    };
    s.shared_relic_bag.remove(id);
    ShopRelic {
        id: Some(id),
        base_cost: merchant_relic_cost(merchant_cost, &mut s.player_rng[PlayerRngType::Shops]),
    }
}

/// `RelicModel.MerchantCost` (`Models/RelicModel.cs:304-315`).
const fn merchant_cost(rarity: RelicRarity) -> i32 {
    match rarity {
        RelicRarity::Common => 175,
        RelicRarity::Uncommon => 225,
        RelicRarity::Rare => 275,
        RelicRarity::Shop => 200,
        RelicRarity::Starter | RelicRarity::Event | RelicRarity::Ancient => 999_999_999,
    }
}

/// A `MerchantPotionEntry` for `id` with its cost: base 50/75/100 by rarity, randomized unless
/// in test mode.
fn potion_entry(s: &mut RunState, id: &'static str) -> ShopPotion {
    let rarity = potion_rarity(id).expect("a pool potion");
    let base = match rarity {
        PotionRarity::Rare => 100,
        PotionRarity::Uncommon => 75,
        PotionRarity::Common => 50,
    };
    let base_cost = if s.test_mode {
        base
    } else {
        merchant_potion_cost(base, &mut s.player_rng[PlayerRngType::Shops])
    };
    ShopPotion {
        id: Some(id),
        base_cost,
    }
}
