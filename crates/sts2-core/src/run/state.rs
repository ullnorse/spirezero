//! `RunState` plus the single player's run-level state, as plain data.

use super::RunError;
use super::cards::{self, RunCard};
use super::hooks;
use super::map::{ActMap, MapCoord, MapError, MapPointType};
use super::modifiers::Modifier;
use super::odds::{CardRarityOdds, PotionRewardOdds, UnknownMapPointOdds};
use super::relic_bag::RelicGrabBag;
use super::relics::RunRelic;
use super::rewards::PendingReward;
use super::rooms::RoomSet;
use super::unlock::UnlockState;
use crate::content::cards::CardType;
use crate::content::characters::Character;
use crate::content::enchantments::{Enchantment, EnchantmentKind};
use crate::decimal::Decimal;
use crate::rng::{PlayerRngSet, Rng, RunRngSet};

/// `KillWithoutCheckingWinCondition` gives up after this many prevented deaths in a row.
const MAX_DEATH_PREVENTIONS: u32 = 10;

/// `HealRestSiteOption.GetBaseHealAmount`: `MaxHp × 0.3m`.
const REST_HEAL: Decimal = Decimal::new(3, 1);

/// `RoomType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoomType {
    /// `RoomType.Monster`.
    Monster,
    /// `RoomType.Elite`.
    Elite,
    /// `RoomType.Boss`.
    Boss,
    /// `RoomType.Event`.
    Event,
    /// `RoomType.Shop`.
    Shop,
    /// `RoomType.Treasure`.
    Treasure,
    /// `RoomType.RestSite`.
    RestSite,
    /// `RoomType.Map`.
    Map,
}

impl RoomType {
    /// The C# enum member name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Monster => "Monster",
            Self::Elite => "Elite",
            Self::Boss => "Boss",
            Self::Event => "Event",
            Self::Shop => "Shop",
            Self::Treasure => "Treasure",
            Self::RestSite => "RestSite",
            Self::Map => "Map",
        }
    }

    /// The C# enum value (`Rooms/RoomType.cs`; `Unassigned` is 0).
    #[must_use]
    pub const fn enum_index(self) -> u8 {
        match self {
            Self::Monster => 1,
            Self::Elite => 2,
            Self::Boss => 3,
            Self::Treasure => 4,
            Self::Shop => 5,
            Self::Event => 6,
            Self::RestSite => 7,
            Self::Map => 8,
        }
    }

    /// `RoomTypeExtensions.IsCombatRoom`: Monster, Elite or Boss.
    #[must_use]
    pub const fn is_combat_room(self) -> bool {
        matches!(self, Self::Monster | Self::Elite | Self::Boss)
    }

    /// The room class the game creates for this type (`RunManager.CreateRoom`).
    #[must_use]
    pub const fn room_class(self) -> &'static str {
        match self {
            Self::Monster | Self::Elite | Self::Boss => "CombatRoom",
            Self::Event => "EventRoom",
            Self::Shop => "MerchantRoom",
            Self::Treasure => "TreasureRoom",
            Self::RestSite => "RestSiteRoom",
            Self::Map => "MapRoom",
        }
    }

    /// `RunManager.RollRoomTypeFor` for a fixed point type; `None` for `Unknown` (a `?` rolls on
    /// `UnknownMapPoint`) and `Unassigned`.
    #[must_use]
    pub const fn for_point(point: MapPointType) -> Option<Self> {
        match point {
            MapPointType::Shop => Some(Self::Shop),
            MapPointType::Treasure => Some(Self::Treasure),
            MapPointType::RestSite => Some(Self::RestSite),
            MapPointType::Monster => Some(Self::Monster),
            MapPointType::Elite => Some(Self::Elite),
            MapPointType::Boss => Some(Self::Boss),
            MapPointType::Ancient => Some(Self::Event),
            MapPointType::Unknown | MapPointType::Unassigned => None,
        }
    }
}

/// One `MapPointHistoryEntry`: what a visited point was and which room it became.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapPointHistoryEntry {
    /// The point's type.
    pub point_type: MapPointType,
    /// `None` only for a point whose room was never created.
    pub room_type: Option<RoomType>,
    /// The room's model (`ModelId.Entry` of the encounter or event), if any.
    pub model: Option<&'static str>,
}

/// The room the player is in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Room {
    /// An ancient's event room (`EventRoom` of the act's `Ancient` start point, `PullAncient`),
    /// with the ancient's `ModelId.Entry`; a stub while the ancient is not ported.
    Ancient {
        /// The ancient's `ModelId.Entry`.
        id: &'static str,
    },
    /// The map screen (`MapRoom`), after a room is finished.
    Map,
    /// A combat room, also while its rewards are shown; the fight itself lives in `Run`.
    Combat {
        /// The room type.
        room_type: RoomType,
        /// The encounter's `ModelId.Entry`.
        encounter: &'static str,
    },
    /// An event room (`EventRoom`) of a `?` point, with its event's `ModelId.Entry`; a stub
    /// while the event is not ported.
    Event {
        /// The event's `ModelId.Entry`.
        id: &'static str,
    },
    /// A rest site (`RestSiteRoom`).
    RestSite,
    /// A treasure room (`TreasureRoom`).
    Treasure,
    /// A merchant room (`MerchantRoom`).
    Shop,
    /// A room the engine does not model yet (an unported event). `proceed` returns to the map;
    /// the run state no longer matches the game's from here on.
    Unsupported {
        /// The point's type.
        point_type: MapPointType,
        /// The room type it would have.
        room_type: RoomType,
    },
}

/// Everything about a run that outlives a combat (`RunState` and the `Player`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunState {
    /// The player's `Character`.
    pub character: Character,
    /// The ascension level.
    pub ascension: u8,
    /// `TestMode.IsOn` (off in the game; the referee's sts2-cli turns it on). Of the ported
    /// rules only merchant potion prices read it: in test mode they are not randomized.
    pub test_mode: bool,
    /// What the profile has unlocked.
    pub unlock: UnlockState,
    /// `RunRngSet`: the 12 run streams, including the combat ones. A fight owns them while it
    /// runs (this is then a placeholder; read [`super::Run::rng`]).
    pub rng: RunRngSet,
    /// The player's `PlayerRngSet` (slot 0).
    pub player_rng: PlayerRngSet,
    /// `PlayerOdds.CardRarity`: the card-reward pity.
    pub card_rarity_odds: CardRarityOdds,
    /// `PlayerOdds.PotionReward`.
    pub potion_reward_odds: PotionRewardOdds,
    /// `RunOddsSet.UnknownMapPoint`: the `?` room odds.
    pub unknown_odds: UnknownMapPointOdds,
    /// `CurrentActIndex`.
    pub act_index: usize,
    /// Every act's rooms, rolled at run start.
    pub acts: Vec<RoomSet>,
    /// The current act's map.
    pub map: ActMap,
    /// `VisitedMapCoords` of the current act; the last is `CurrentMapCoord`.
    pub visited: Vec<MapCoord>,
    /// `MapPointHistory` of the current act.
    pub history: Vec<MapPointHistoryEntry>,
    /// `ActFloor`: the current point's row + 1.
    pub act_floor: usize,
    /// `VisitedEventIds`.
    pub visited_event_ids: Vec<&'static str>,
    /// The room the player is in.
    pub room: Room,
    /// `CurrentHp`.
    pub hp: i32,
    /// `MaxHp`.
    pub max_hp: i32,
    /// `Gold`.
    pub gold: i32,
    /// `MaxEnergy`.
    pub max_energy: i32,
    /// The deck, in deck order.
    pub deck: Vec<RunCard>,
    /// The relics in the order obtained.
    pub relics: Vec<RunRelic>,
    /// The potion belt by slot (potion ids; `None` is an empty slot).
    pub potions: Vec<Option<&'static str>>,
    /// The shared relic grab bag.
    pub shared_relic_bag: RelicGrabBag,
    /// The player's relic grab bag.
    pub player_relic_bag: RelicGrabBag,
    /// `ExtraFields.CardShopRemovalsUsed`.
    pub card_shop_removals_used: i32,
    /// The point of the current act's map that carries a `SpoilsMap` quest marker
    /// (`MapPoint.Quests`, `SpoilsMap.AfterMapGenerated`); entering its treasure room completes the
    /// quest ([`super::Run`]'s treasure room).
    pub spoils_quest: Option<MapCoord>,
    /// `RunState.Modifiers`: the custom run's modifiers, in the order they were ticked.
    pub modifiers: Vec<Modifier>,
}

impl RunState {
    /// `TotalFloor`: the number of map points entered so far (Neow included).
    #[must_use]
    pub const fn total_floor(&self) -> usize {
        self.history.len()
    }

    /// `RunManager.GenerateMap` (`RunManager.cs:799-831`), outside a save load: the current
    /// act's map from its own stream (`ActModel.CreateMap` → `StandardActMap.CreateFor`), through
    /// [`hooks::modify_generated_map`]; then the visited coords the new map lacks are dropped
    /// (`RemoveStaleVisitedMapCoords`). `AfterMapGenerated` only places quest markers: a
    /// `SpoilsMap`'s on its `SpoilsCoord`, noted by [`hooks::modify_generated_map_late`], if the
    /// map has the point (`SpoilsMap.cs:82-97`); the old map's markers go with it. Every run starts
    /// with Neow, so Act 1's start point stays an Ancient. Run start builds Act 1's map directly:
    /// no listener exists yet.
    ///
    /// # Errors
    /// Where the game throws (see [`MapError`]).
    pub fn generate_map(&mut self) -> Result<(), MapError> {
        let act = self.act_index;
        let mut map = ActMap::generate(&self.rng, act, self.ascension)?;
        self.spoils_quest = None;
        hooks::modify_generated_map(self, &mut map, act)?;
        self.spoils_quest = self.spoils_quest.filter(|&c| map.has_point(c));
        self.map = map;
        let map = &self.map;
        self.visited.retain(|&c| map.has_point(c));
        Ok(())
    }

    /// `CurrentMapCoord`.
    #[must_use]
    pub fn coord(&self) -> Option<MapCoord> {
        self.visited.last().copied()
    }

    /// The current act's rooms.
    #[must_use]
    pub fn act(&self) -> &RoomSet {
        &self.acts[self.act_index]
    }

    pub(crate) fn act_mut(&mut self) -> &mut RoomSet {
        &mut self.acts[self.act_index]
    }

    /// The room type of the current room (`Map` on the map screen), or `None` for a stub.
    #[must_use]
    pub const fn room_type(&self) -> Option<RoomType> {
        match &self.room {
            Room::Ancient { .. } | Room::Event { .. } => Some(RoomType::Event),
            Room::Map => Some(RoomType::Map),
            Room::Combat { room_type, .. } | Room::Unsupported { room_type, .. } => {
                Some(*room_type)
            }
            Room::RestSite => Some(RoomType::RestSite),
            Room::Treasure => Some(RoomType::Treasure),
            Room::Shop => Some(RoomType::Shop),
        }
    }

    /// `PlayerCmd.GainGold` (`Commands/PlayerCmd.cs:140-177`): `ModifyGoldGained`, then
    /// `AfterModifyingGoldGained`, then if the amount is positive `Gold += (int)amount` and
    /// `AfterGoldGained`.
    pub fn gain_gold(&mut self, amount: Decimal) {
        let (amount, modifiers) = hooks::modify_gold_gained(self, amount);
        hooks::after_modifying_gold_gained(self, &modifiers, amount);
        if amount > Decimal::ZERO {
            self.gold += amount.to_int_truncating();
            hooks::after_gold_gained(self);
        }
    }

    /// `PlayerCmd.LoseGold`: never below 0.
    pub fn lose_gold(&mut self, amount: i32) {
        self.gold = (self.gold - amount).max(0);
    }

    /// `CreatureCmd.Damage` on the player outside combat (`Commands/CreatureCmd.cs:258-432`):
    /// no block, `ModifyHpLost` ([`hooks::modify_hp_lost_out_of_combat`]), then HP drops by the
    /// truncated amount, not below 0 (`LoseHpInternal`); a blow that takes the last HP
    /// (`WasTargetKilled`) ends in [`Self::kill`].
    pub fn damage(&mut self, amount: i32) {
        let lost = hooks::modify_hp_lost_out_of_combat(self, Decimal::from(amount));
        let killed = self.hp > 0 && lost >= Decimal::from(self.hp);
        self.hp = (self.hp - lost.to_int_truncating()).max(0);
        if killed {
            self.kill();
        }
    }

    /// `CreatureCmd.Heal`: `(int)Math.Min(hp + amount, maxHp)` (`Creature.HealInternal`).
    pub fn heal(&mut self, amount: Decimal) {
        let healed = (Decimal::from(self.hp) + amount).min(Decimal::from(self.max_hp));
        self.hp = healed.to_int_truncating();
    }

    /// `HealRestSiteOption.GetHealAmount`: `Hook.ModifyRestSiteHealAmount` of `MaxHp × 0.3m`.
    #[must_use]
    pub fn rest_site_heal_amount(&self) -> Decimal {
        hooks::modify_rest_site_heal_amount(self, Decimal::from(self.max_hp) * REST_HEAL)
    }

    /// `HealRestSiteOption.ExecuteRestSiteHeal` (`Entities.RestSite/HealRestSiteOption.cs:105-113`),
    /// which `PlayerCmd.MimicRestSiteHeal` also runs (`isMimicked`, `DenseVegetation`'s Rest):
    /// heal `MaxHp × 0.3m` through `ModifyRestSiteHealAmount` (truncated with the current HP),
    /// then `AfterRestSiteHeal`. Returns the rewards `ModifyRestSiteHealRewards` adds, which the
    /// caller offers (`OfferCustom`; nothing is shown for none).
    #[must_use]
    pub fn rest_site_heal(&mut self, is_mimicked: bool) -> Vec<PendingReward> {
        let amount = self.rest_site_heal_amount();
        self.heal(amount);
        hooks::after_rest_site_heal(self, is_mimicked);
        hooks::modify_rest_site_heal_rewards(self, is_mimicked)
    }

    /// `CreatureCmd.LoseMaxHp` (`CreatureCmd.cs:863-879`): HP above the new max is lost as
    /// unblockable damage, then max HP becomes `max(1, max - amount)`.
    pub fn lose_max_hp(&mut self, amount: i32) {
        let new_max = self.max_hp - amount;
        if new_max < self.hp {
            self.damage(self.hp - new_max);
        }
        self.max_hp = new_max.max(1);
    }

    /// `CreatureCmd.Kill` on the player outside combat, not forced
    /// (`KillWithoutCheckingWinCondition`, `CreatureCmd.cs:514-603`): HP drops to 0, then
    /// `Hook.ShouldDie`; a preventer's `AfterPreventingDeath` runs, and the kill repeats while
    /// the player is still dead. `BeforeDeath`/`AfterDeath` have no ported run-level listener.
    ///
    /// # Panics
    /// If deaths keep being prevented (`InvalidOperationException` in the game).
    pub fn kill(&mut self) {
        for _ in 0..=MAX_DEATH_PREVENTIONS {
            self.hp = 0;
            let Some(preventer) = hooks::should_die(self) else {
                return;
            };
            hooks::after_preventing_death(self, preventer);
            if self.hp > 0 {
                return;
            }
        }
        panic!("something is continually preventing the player from being killed");
    }

    /// `CardPileCmd.AddCursesToDeck` / `CardPileCmd.Add(card, Deck)`: the card, as
    /// `ModifyCardBeingAddedToDeck` returns it, joins the end of the deck; then
    /// `AfterCardChangedPiles`.
    pub fn add_card(&mut self, card: RunCard) {
        let card = hooks::modify_card_being_added_to_deck(self, card);
        self.deck.push(card);
        hooks::after_card_added_to_deck(self, card);
    }

    /// `CardPileCmd.RemoveFromDeck`: `BeforeCardRemoved`, then the card leaves.
    pub fn remove_card(&mut self, index: usize) {
        hooks::before_card_removed(self, self.deck[index]);
        self.deck.remove(index);
    }

    /// `CardPileCmd.RemoveFromDeck(cards)` for several deck indices: each in the given order,
    /// as the indices were before any removal.
    pub fn remove_cards(&mut self, indices: &[usize]) {
        for (n, &i) in indices.iter().enumerate() {
            let shift = indices[..n].iter().filter(|&&j| j < i).count();
            self.remove_card(i - shift);
        }
    }

    /// `CardCmd.TransformToRandom(card, rng)` on a deck card (`Commands/CardCmd.cs:325-460`,
    /// `Factories/CardFactory.cs:170-208`): a random card of the original's pool (the colorless
    /// pool for a Quest, Event, Ancient or Token card) other than the original, Common, Uncommon
    /// or Rare unless the original is a Status or Curse, drawn with `NextItem` on `rng`. The
    /// original leaves the deck and the replacement, unupgraded and through
    /// `ModifyCardBeingAddedToDeck`, joins its end.
    ///
    /// # Errors
    /// [`RunError::UnsupportedCard`] if the original's pool is not modeled (curses, status
    /// cards).
    ///
    /// # Panics
    /// If the card is not transformable or the pool has no option (the game throws).
    pub fn transform_to_random(&mut self, index: usize, rng: &mut Rng) -> Result<(), RunError> {
        let original = self.deck[index];
        assert!(
            original.is_removable(),
            "only transformable cards transform"
        );
        let options = cards::transformation_options(original)
            .ok_or(RunError::UnsupportedCard(original.id))?;
        let id = *rng.next_item(&options).expect("a transformation option");
        self.transform_to(index, RunCard::new(id));
        Ok(())
    }

    /// `CardCmd.Transform(transformations, rng)` with a random replacement for each deck card of
    /// `indices` (`Commands/CardCmd.cs:374-460`): the replacements are drawn in the given order
    /// (as [`RunState::transform_to_random`]), then [`RunState::transform_many`].
    ///
    /// # Errors
    /// As [`RunState::transform_to_random`]; the deck is then unchanged (`rng` may have moved).
    ///
    /// # Panics
    /// If a card is not transformable or its pool has no option (the game throws).
    pub fn transform_many_to_random(
        &mut self,
        indices: &[usize],
        rng: &mut Rng,
    ) -> Result<(), RunError> {
        let mut replacements = Vec::with_capacity(indices.len());
        for &i in indices {
            let original = self.deck[i];
            assert!(
                original.is_removable(),
                "only transformable cards transform"
            );
            let options = cards::transformation_options(original)
                .ok_or(RunError::UnsupportedCard(original.id))?;
            let id = *rng.next_item(&options).expect("a transformation option");
            replacements.push((i, RunCard::new(id)));
        }
        self.transform_many(&replacements);
        Ok(())
    }

    /// `CardCmd.Transform(transformations, ..)` on the deck (`Commands/CardCmd.cs:374-460`) for
    /// `(deck index, replacement)` pairs, the indices the deck's before any change: in the given
    /// order each original's index in the deck as it is then (the earlier originals already
    /// gone) is noted and it leaves the deck; then the replacements, stably sorted by those
    /// indices (`PileIndexSort`; `List.Sort` is an insertion sort for so few), join the deck's
    /// end through `ModifyCardBeingAddedToDeck`.
    pub fn transform_many(&mut self, transformations: &[(usize, RunCard)]) {
        let mut noted: Vec<(usize, RunCard)> = Vec::with_capacity(transformations.len());
        for (n, &(i, replacement)) in transformations.iter().enumerate() {
            let shift = transformations[..n].iter().filter(|&&(j, _)| j < i).count();
            noted.push((i - shift, replacement));
        }
        let mut removed: Vec<usize> = transformations.iter().map(|&(i, _)| i).collect();
        removed.sort_unstable();
        for &i in removed.iter().rev() {
            self.deck.remove(i);
        }
        noted.sort_by_key(|&(i, _)| i);
        for (_, replacement) in noted {
            let card = hooks::modify_card_being_added_to_deck(self, replacement);
            self.deck.push(card);
            hooks::after_card_added_to_deck_by_relics(self, card);
        }
    }

    /// Per deck card of `indices` in the given order (`Astrolabe`): a random replacement drawn as
    /// in [`RunState::transform_to_random`], upgraded, then [`RunState::transform_to`]. Each
    /// transform moves the later cards, so the indices are the deck's before the first one.
    ///
    /// # Errors
    /// As [`RunState::transform_to_random`]; the cards transformed so far stay transformed.
    ///
    /// # Panics
    /// If a card is not transformable or its pool has no option (the game throws).
    pub fn transform_each_to_random_upgraded(
        &mut self,
        indices: &[usize],
        rng: &mut Rng,
    ) -> Result<(), RunError> {
        let mut indices = indices.to_vec();
        for n in 0..indices.len() {
            let i = indices[n];
            let original = self.deck[i];
            assert!(
                original.is_removable(),
                "only transformable cards transform"
            );
            let options = cards::transformation_options(original)
                .ok_or(RunError::UnsupportedCard(original.id))?;
            let mut replacement = RunCard::new(
                rng.next_item(&options)
                    .copied()
                    .expect("a transformation option"),
            );
            replacement.upgrade();
            self.transform_to(i, replacement);
            for j in &mut indices[n + 1..] {
                if *j > i {
                    *j -= 1;
                }
            }
        }
        Ok(())
    }

    /// `CardCmd.Transform(card, replacement)` on a deck card: the original leaves the deck
    /// (no `BeforeCardRemoved`), the replacement goes through `ModifyCardBeingAddedToDeck` and
    /// joins the end of the deck.
    pub fn transform_to(&mut self, index: usize, replacement: RunCard) {
        self.deck.remove(index);
        let card = hooks::modify_card_being_added_to_deck(self, replacement);
        self.deck.push(card);
        // `AfterCardChangedPiles(replacement, Deck, null)`: `Hoarder` only copies a card that
        // came from no pile, so a transformed card is not copied.
        hooks::after_card_added_to_deck_by_relics(self, card);
    }

    /// `CardCmd.Downgrade` on a deck card ([`RunCard::downgrade`]).
    pub fn downgrade(&mut self, index: usize) {
        self.deck[index].downgrade();
    }

    /// `CardCmd.Enchant(enchantment, card, amount)` on a deck card (`Commands/CardCmd.cs:536-563`):
    /// a new enchantment, or `amount` more on the same kind. `OnEnchant` only changes combat
    /// card state (keywords), which the combat clone of the deck card gets from
    /// [`modify_card`](crate::content::enchantments::modify_card).
    ///
    /// # Errors
    /// [`RunError::UnsupportedCard`] if the card is not ported (its keywords and cost decide
    /// `CanEnchant`).
    ///
    /// # Panics
    /// If the card cannot take the enchantment (the game throws).
    pub fn enchant(
        &mut self,
        index: usize,
        kind: EnchantmentKind,
        amount: i32,
    ) -> Result<(), RunError> {
        let card = &mut self.deck[index];
        assert!(card.can_enchant(kind)?, "{} cannot take {kind:?}", card.id);
        if let Some(e) = &mut card.enchantment {
            e.amount += amount;
        } else {
            card.enchantment = Some(Enchantment::new(kind, amount));
            // `ModifyCard` → `RecalculateForUpgradeOrEnchant` (`EnchantmentModel.cs:363`).
            card.calculated_level = None;
        }
        Ok(())
    }

    /// `PotionCmd.Discard`: the slot empties.
    pub fn discard_potion(&mut self, slot: usize) {
        self.potions[slot] = None;
    }

    /// `Player.Potions`: the potions in the belt, in slot order, with their slots.
    pub fn potions_held(&self) -> impl Iterator<Item = (usize, &'static str)> + '_ {
        self.potions
            .iter()
            .enumerate()
            .filter_map(|(slot, p)| p.map(|id| (slot, id)))
    }

    /// `CreatureCmd.GainMaxHp`: max HP up, then heal by the same amount.
    pub fn gain_max_hp(&mut self, amount: i32) {
        self.max_hp += amount;
        self.hp = (self.hp + amount).min(self.max_hp);
    }

    /// `PotionCmd.TryToProcure` outside combat: `Hook.ShouldProcurePotion` (`Sozu`), then
    /// `Player.AddPotionInternal` into the first empty slot; `false` if vetoed or the belt is
    /// full. `AfterPotionProcured` has no run-level listener.
    pub fn procure_potion(&mut self, id: &'static str) -> bool {
        if !hooks::should_procure_potion(self) {
            return false;
        }
        let Some(slot) = self.potions.iter_mut().find(|p| p.is_none()) else {
            return false;
        };
        *slot = Some(id);
        true
    }

    /// `PotionCmd.TryToProcure(potion, player, slot)` outside combat: as
    /// [`RunState::procure_potion`] into the given slot; `false` if vetoed or the slot is filled.
    pub fn procure_potion_at(&mut self, id: &'static str, slot: usize) -> bool {
        if !hooks::should_procure_potion(self) || self.potions[slot].is_some() {
            return false;
        }
        self.potions[slot] = Some(id);
        true
    }

    /// `RelicCmd.Melt` (`Commands/RelicCmd.cs:89-93`): the wax relic at `index` is melted
    /// (`Player.MeltRelicInternal`; it stays owned), then its `AfterRemoved` (no relic overrides
    /// it).
    ///
    /// # Panics
    /// If the relic is not wax or is already melted (the game throws).
    pub fn melt_relic(&mut self, index: usize) {
        let relic = &mut self.relics[index];
        assert!(
            relic.wax && !relic.melted,
            "only an unmelted wax relic melts"
        );
        relic.melted = true;
    }

    /// `PlayerCmd.GainMaxPotionCount`: that many empty slots are added.
    pub fn gain_max_potion_count(&mut self, count: usize) {
        self.potions.extend(std::iter::repeat_n(None, count));
    }

    /// Deck indices of the removable cards as `FromDeckForRemoval` lists them
    /// (`Commands/CardSelectCmd.cs:739-744`): curses first, then deck order.
    #[must_use]
    pub fn removable_cards(&self) -> Vec<usize> {
        let mut cards: Vec<usize> = (0..self.deck.len())
            .filter(|&i| self.deck[i].is_removable())
            .collect();
        cards.sort_by_key(|&i| self.deck[i].card_type() != Some(CardType::Curse));
        cards
    }

    /// Deck indices of the cards `FromDeckForTransformation` lists, in deck order: not Quest
    /// cards, and transformable (`IsRemovable` in the deck).
    #[must_use]
    pub fn transformable_cards(&self) -> Vec<usize> {
        (0..self.deck.len())
            .filter(|&i| {
                let c = self.deck[i];
                c.card_type() != Some(CardType::Quest) && c.is_removable()
            })
            .collect()
    }

    /// Deck indices of the cards `FromDeckForEnchantment` lists, in deck order: those that can
    /// take `kind`.
    ///
    /// # Errors
    /// [`RunError::UnsupportedCard`] for a deck card that is not ported.
    pub fn enchantable_cards(&self, kind: EnchantmentKind) -> Result<Vec<usize>, RunError> {
        let mut cards = Vec::new();
        for (i, card) in self.deck.iter().enumerate() {
            if card.can_enchant(kind)? {
                cards.push(i);
            }
        }
        Ok(cards)
    }

    /// Deck indices of the upgradable cards, in deck order (`FromDeckForUpgrade`).
    #[must_use]
    pub fn upgradable_cards(&self) -> Vec<usize> {
        (0..self.deck.len())
            .filter(|&i| self.deck[i].is_upgradable())
            .collect()
    }
}
