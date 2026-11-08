//! The relic pools and `RelicGrabBag` (`Runs/RelicGrabBag.cs`), populated at run start.

use super::state::RunState;
use crate::content::characters::Character;
use crate::content::relics::RelicKind;
use crate::rng::Rng;

/// `RelicRarity`, as far as the pools in scope use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RelicRarity {
    /// `RelicRarity.Starter`.
    Starter,
    /// `RelicRarity.Common`.
    Common,
    /// `RelicRarity.Uncommon`.
    Uncommon,
    /// `RelicRarity.Rare`.
    Rare,
    /// `RelicRarity.Shop`.
    Shop,
    /// `RelicRarity.Event`.
    Event,
    /// `RelicRarity.Ancient`.
    Ancient,
}

use RelicRarity::{Ancient, Common, Event, Rare, Shop, Uncommon};

/// The rarities a player bag keeps (`RelicGrabBag._rarities`).
pub(crate) const BAG_RARITIES: [RelicRarity; 4] = [Common, Uncommon, Rare, Shop];

/// `SharedRelicPool.GenerateAllRelics()` in declaration order, with each relic's `Rarity`.
pub const SHARED_RELIC_POOL: [(&str, RelicRarity); 118] = [
    ("AKABEKO", Uncommon),
    ("AMETHYST_AUBERGINE", Common),
    ("ANCHOR", Common),
    ("ART_OF_WAR", Rare),
    ("BAG_OF_MARBLES", Common),
    ("BAG_OF_PREPARATION", Common),
    ("BEATING_REMNANT", Rare),
    ("BELLOWS", Rare),
    ("BELT_BUCKLE", Shop),
    ("BLOOD_VIAL", Common),
    ("BOOK_OF_FIVE_RINGS", Common),
    ("BOWLER_HAT", Uncommon),
    ("BREAD", Shop),
    ("BRONZE_SCALES", Common),
    ("BURNING_STICKS", Shop),
    ("CANDELABRA", Uncommon),
    ("CAPTAINS_WHEEL", Rare),
    ("CAULDRON", Shop),
    ("CENTENNIAL_PUZZLE", Common),
    ("CHANDELIER", Rare),
    ("CHEMICAL_X", Shop),
    ("CLOAK_CLASP", Rare),
    ("DINGY_RUG", Shop),
    ("DOLLYS_MIRROR", Shop),
    ("DRAGON_FRUIT", Shop),
    ("ETERNAL_FEATHER", Uncommon),
    ("FESTIVE_POPPER", Common),
    ("FRESNEL_LENS", Event),
    ("FROZEN_EGG", Rare),
    ("GAMBLING_CHIP", Rare),
    ("GAME_PIECE", Rare),
    ("GHOST_SEED", Shop),
    ("GIRYA", Rare),
    ("GNARLED_HAMMER", Shop),
    ("GORGET", Common),
    ("GREMLIN_HORN", Uncommon),
    ("HAPPY_FLOWER", Common),
    ("HORN_CLEAT", Uncommon),
    ("ICE_CREAM", Rare),
    ("INTIMIDATING_HELMET", Rare),
    ("JOSS_PAPER", Uncommon),
    ("JUZU_BRACELET", Common),
    ("KIFUDA", Shop),
    ("KUNAI", Rare),
    ("KUSARIGAMA", Uncommon),
    ("LANTERN", Common),
    ("LASTING_CANDY", Uncommon),
    ("LAVA_LAMP", Shop),
    ("LEES_WAFFLE", Shop),
    ("LETTER_OPENER", Uncommon),
    ("LIZARD_TAIL", Rare),
    ("LOOMING_FRUIT", Ancient),
    ("LUCKY_FYSH", Uncommon),
    ("MANGO", Rare),
    ("MEAL_TICKET", Common),
    ("MEAT_ON_THE_BONE", Rare),
    ("MEMBERSHIP_CARD", Shop),
    ("MERCURY_HOURGLASS", Uncommon),
    ("MINIATURE_CANNON", Uncommon),
    ("MINIATURE_TENT", Shop),
    ("MOLTEN_EGG", Rare),
    ("MUMMIFIED_HAND", Rare),
    ("MYSTIC_LIGHTER", Shop),
    ("NUNCHAKU", Uncommon),
    ("ODDLY_SMOOTH_STONE", Common),
    ("OLD_COIN", Rare),
    ("ORICHALCUM", Uncommon),
    ("ORNAMENTAL_FAN", Uncommon),
    ("ORRERY", Shop),
    ("PANTOGRAPH", Uncommon),
    ("PARRYING_SHIELD", Uncommon),
    ("PEAR", Uncommon),
    ("PEN_NIB", Uncommon),
    ("PENDULUM", Common),
    ("PERMAFROST", Uncommon),
    ("PETRIFIED_TOAD", Uncommon),
    ("PLANISPHERE", Uncommon),
    ("POCKETWATCH", Rare),
    ("POTION_BELT", Common),
    ("PRAYER_WHEEL", Rare),
    ("PUNCH_DAGGER", Shop),
    ("RAINBOW_RING", Rare),
    ("RAZOR_TOOTH", Rare),
    ("RED_MASK", Common),
    ("REGAL_PILLOW", Common),
    ("REPTILE_TRINKET", Uncommon),
    ("RINGING_TRIANGLE", Shop),
    ("RIPPLE_BASIN", Uncommon),
    ("ROYAL_STAMP", Shop),
    ("SCREAMING_FLAGON", Shop),
    ("SHOVEL", Rare),
    ("SHURIKEN", Rare),
    ("SLING_OF_COURAGE", Shop),
    ("SPARKLING_ROUGE", Uncommon),
    ("STONE_CALENDAR", Rare),
    ("STONE_CRACKER", Uncommon),
    ("STRAWBERRY", Common),
    ("STRIKE_DUMMY", Common),
    ("STURDY_CLAMP", Rare),
    ("THE_ABACUS", Shop),
    ("THE_COURIER", Rare),
    ("TINY_MAILBOX", Uncommon),
    ("TOOLBOX", Shop),
    ("TOXIC_EGG", Rare),
    ("TUNGSTEN_ROD", Rare),
    ("TUNING_FORK", Uncommon),
    ("UNCEASING_TOP", Rare),
    ("UNSETTLING_LAMP", Rare),
    ("VAJRA", Common),
    ("VAMBRACE", Uncommon),
    ("VENERABLE_TEA_SET", Common),
    ("VERY_HOT_COCOA", Ancient),
    ("VEXING_PUZZLEBOX", Rare),
    ("WAR_PAINT", Common),
    ("WHETSTONE", Common),
    ("WHITE_BEAST_STATUE", Rare),
    ("WHITE_STAR", Rare),
    ("WING_CHARM", Shop),
];

/// A populated `RelicGrabBag`: one list per rarity, in first-seen order, each shuffled.
///
/// The multiplayer `_mpFallbackDequeue` (filled only by `MoveToFallback` at a shared treasure
/// chest) is always empty in single player and is not modelled.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelicGrabBag {
    /// `_deques` in insertion order (the order the game's `Dictionary` enumerates them).
    pub deques: Vec<(RelicRarity, Vec<&'static str>)>,
    /// `_refreshAllowed`.
    pub refresh_allowed: bool,
    /// `_originalRelics`: the populated list, before bucketing and shuffling.
    pub original: Vec<(&'static str, RelicRarity)>,
}

/// `relic.IsAllowed(runState)`. A relic with no file keeps the base `true`: every pool relic
/// whose `IsAllowed` is overridden is ported (`tests::every_culled_pool_relic_is_ported`).
#[must_use]
pub fn is_allowed(id: &str, s: &RunState) -> bool {
    RelicKind::from_id(id).is_none_or(|kind| kind.model().is_allowed(s))
}

/// The next bucket when one has nothing to give (`GetAvailableDeque`): Shop → Common →
/// Uncommon → Rare, then none.
const fn next_rarity(rarity: RelicRarity) -> Option<RelicRarity> {
    match rarity {
        Shop => Some(Common),
        Common => Some(Uncommon),
        Uncommon => Some(Rare),
        _ => None,
    }
}

impl RelicGrabBag {
    /// `Populate(relics, rng)`: bucket by rarity in first-seen order, then `UnstableShuffle`
    /// each bucket in that order. No rarity is filtered out here.
    fn populate(
        relics: Vec<(&'static str, RelicRarity)>,
        refresh_allowed: bool,
        rng: &mut Rng,
    ) -> Self {
        let mut deques: Vec<(RelicRarity, Vec<&'static str>)> = Vec::new();
        for &(id, rarity) in &relics {
            match deques.iter_mut().find(|(r, _)| *r == rarity) {
                Some((_, list)) => list.push(id),
                None => deques.push((rarity, vec![id])),
            }
        }
        for (_, list) in &mut deques {
            crate::shuffle::unstable_shuffle(list, rng);
        }
        Self {
            deques,
            refresh_allowed,
            original: relics,
        }
    }

    /// The run's shared bag (`RunManager.InitializeNewRun`, `RelicGrabBag(refreshAllowed: true)`):
    /// the unlocked shared pool, every rarity kept.
    pub fn shared(rng: &mut Rng) -> Self {
        Self::populate(SHARED_RELIC_POOL.to_vec(), true, rng)
    }

    /// A `character`'s bag (`Populate(player, rng)`): shared pool then character pool, keeping
    /// only Common, Uncommon, Rare and Shop.
    pub fn player(character: Character, rng: &mut Rng) -> Self {
        let relics: Vec<(&'static str, RelicRarity)> = SHARED_RELIC_POOL
            .iter()
            .chain(character.relic_pool())
            .filter(|(_, rarity)| BAG_RARITIES.contains(rarity))
            .copied()
            .collect();
        Self::populate(relics, false, rng)
    }

    /// `PullFromFront(rarity, runState)` (`RelicGrabBag.cs:116-146`): the first relic of the
    /// bucket [`Self::available_deque`] picks, `None` if there is none.
    pub fn pull_from_front(&mut self, rarity: RelicRarity, s: &RunState) -> Option<&'static str> {
        self.pull_from_front_where(rarity, s, |_| true)
    }

    /// `PullFromFront(rarity, filter, runState)` (`RelicGrabBag.cs:129-146`): the first relic
    /// passing `filter` in the bucket [`Self::available_deque`] picks with that filter.
    pub fn pull_from_front_where(
        &mut self,
        rarity: RelicRarity,
        s: &RunState,
        filter: impl Fn(&str) -> bool,
    ) -> Option<&'static str> {
        let list = self.available_deque(rarity, s, &filter)?;
        let i = list.iter().position(|id| filter(id))?;
        Some(list.remove(i))
    }

    /// `PullFromBack(rarity, filter, runState)` (`RelicGrabBag.cs:156-173`): the last relic
    /// passing `filter` in the bucket [`Self::available_deque`] picks. Returns the relic and
    /// its bucket's rarity (the relic's `Rarity`).
    pub fn pull_from_back(
        &mut self,
        rarity: RelicRarity,
        s: &RunState,
        filter: impl Fn(&str) -> bool,
    ) -> Option<(&'static str, RelicRarity)> {
        let rarity = self.available_rarity(rarity, s, &filter)?;
        let list = self.deque_mut(rarity)?;
        let i = list.iter().rposition(|id| filter(id))?;
        Some((list.remove(i), rarity))
    }

    /// `GetAvailableDeque(rarity, runState, filter)` (`RelicGrabBag.cs:218-243`).
    fn available_deque(
        &mut self,
        rarity: RelicRarity,
        s: &RunState,
        filter: impl Fn(&str) -> bool,
    ) -> Option<&mut Vec<&'static str>> {
        let rarity = self.available_rarity(rarity, s, &filter)?;
        self.deque_mut(rarity)
    }

    /// The rarity of the bucket `GetAvailableDeque` returns: the disallowed relics are culled;
    /// an empty bucket of a refreshable bag is refilled (and culled again); then the first bucket
    /// from `rarity` along Shop → Common → Uncommon → Rare with a relic passing `filter`.
    fn available_rarity(
        &mut self,
        rarity: RelicRarity,
        s: &RunState,
        filter: &impl Fn(&str) -> bool,
    ) -> Option<RelicRarity> {
        self.remove_disallowed(s);
        // `GetDeque` of a rarity the bag never had is a fresh empty list: `RefreshRarity` then
        // creates the bucket, but this call keeps looking at that empty list.
        let had_bucket = self.deque(rarity).is_some();
        if self.deque(rarity).is_none_or(Vec::is_empty) && self.refresh_allowed {
            self.refresh_rarity(rarity);
            self.remove_disallowed(s);
        }
        let mut skip = !had_bucket;
        let mut current = Some(rarity);
        while let Some(r) = current {
            if !skip && self.deque(r).is_some_and(|l| l.iter().any(|id| filter(id))) {
                return Some(r);
            }
            skip = false;
            current = next_rarity(r);
        }
        None
    }

    fn deque(&self, rarity: RelicRarity) -> Option<&Vec<&'static str>> {
        self.deques
            .iter()
            .find(|(r, _)| *r == rarity)
            .map(|(_, list)| list)
    }

    fn deque_mut(&mut self, rarity: RelicRarity) -> Option<&mut Vec<&'static str>> {
        self.deques
            .iter_mut()
            .find(|(r, _)| *r == rarity)
            .map(|(_, list)| list)
    }

    /// `RemoveDisallowedRelicsFromDeques(runState)` (`RelicGrabBag.cs:250-272`): drops every
    /// relic whose `IsAllowed` fails, from every bucket.
    fn remove_disallowed(&mut self, s: &RunState) {
        for (_, list) in &mut self.deques {
            list.retain(|id| is_allowed(id, s));
        }
    }

    /// `RefreshRarity(rarity)` (`RelicGrabBag.cs:322-340`): appends the populated relics of
    /// `rarity` to its bucket in their original order (not shuffled), creating the bucket last
    /// if the bag never had it.
    fn refresh_rarity(&mut self, rarity: RelicRarity) {
        let refill = self
            .original
            .iter()
            .filter(|(_, r)| *r == rarity)
            .map(|(id, _)| *id);
        if let Some((_, list)) = self.deques.iter_mut().find(|(r, _)| *r == rarity) {
            list.extend(refill);
        } else {
            let list: Vec<&'static str> = refill.collect();
            if !list.is_empty() {
                self.deques.push((rarity, list));
            }
        }
    }

    /// `HasAvailableRelics(runState)` (`RelicGrabBag.cs:47-58`): some Common, Uncommon, Rare or
    /// Shop bucket (with fall-through) holds an allowed relic. The game culls (and in a
    /// refreshable bag refills) on the way; every pull culls first and an empty bucket is refilled
    /// whenever it is asked for, so answering without mutating gives the same later draws.
    #[must_use]
    pub fn has_available_relics(&self, s: &RunState) -> bool {
        let mut bag = self.clone();
        BAG_RARITIES
            .iter()
            .any(|&r| bag.available_rarity(r, s, &|_| true).is_some())
    }

    /// `Remove(relic)`: drop every copy of `id` from every bucket.
    pub fn remove(&mut self, id: &str) {
        for (_, list) in &mut self.deques {
            list.retain(|r| *r != id);
        }
    }
}

/// One of the run's two bags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bag {
    /// `RunState.SharedRelicGrabBag`.
    Shared,
    /// `Player.RelicGrabBag`.
    Player,
}

impl RunState {
    /// Runs `f` on one of the bags next to the rest of the run state, which the bag's
    /// `IsAllowed` checks read (the bag is moved out meanwhile; no `IsAllowed` reads a bag).
    pub fn with_relic_bag<R>(
        &mut self,
        bag: Bag,
        f: impl FnOnce(&mut RelicGrabBag, &Self) -> R,
    ) -> R {
        let slot = match bag {
            Bag::Shared => &mut self.shared_relic_bag,
            Bag::Player => &mut self.player_relic_bag,
        };
        let mut taken = std::mem::take(slot);
        let result = f(&mut taken, self);
        match bag {
            Bag::Shared => self.shared_relic_bag = taken,
            Bag::Player => self.player_relic_bag = taken,
        }
        result
    }
}

/// `RelicModel.IsBeforeAct3TreasureChest` (`Models/RelicModel.cs:452-456`): `TotalFloor` below
/// 41 in single player (38 with more players).
#[must_use]
pub const fn is_before_act3_treasure_chest(s: &RunState) -> bool {
    s.total_floor() < ACT3_TREASURE_FLOOR
}

/// The single-player floor of the Act 3 treasure chest.
const ACT3_TREASURE_FLOOR: usize = 41;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_bag_keeps_event_and_ancient_buckets() {
        let mut rng = Rng::new(1);
        let bag = RelicGrabBag::shared(&mut rng);
        let rarities: Vec<RelicRarity> = bag.deques.iter().map(|(r, _)| *r).collect();
        assert_eq!(rarities, [Uncommon, Common, Rare, Shop, Event, Ancient]);
        // n - 1 draws per bucket of n.
        assert_eq!(rng.counter(), 29 + 24 + 34 + 24 + 1);
    }

    #[test]
    fn player_bag_drops_the_starter() {
        let mut rng = Rng::new(1);
        let bag = RelicGrabBag::player(Character::Necrobinder, &mut rng);
        let total: usize = bag.deques.iter().map(|(_, l)| l.len()).sum();
        assert_eq!(total, 115 + 7);
        assert!(
            bag.deques
                .iter()
                .all(|(_, l)| !l.contains(&"BOUND_PHYLACTERY"))
        );
    }

    use crate::run::map::MapPointType;
    use crate::run::state::MapPointHistoryEntry;
    use crate::run::{Act1, Run, UnlockState};

    /// `T6S0`'s run state with `floors` map points entered.
    fn at_floor(floors: usize) -> RunState {
        let mut s = Run::new(
            Character::Necrobinder,
            "T6S0",
            0,
            UnlockState::all(),
            Act1::Overgrowth,
        )
        .expect("T6S0 starts")
        .state;
        s.history = vec![
            MapPointHistoryEntry {
                point_type: MapPointType::Monster,
                room_type: None,
                model: None,
            };
            floors
        ];
        s
    }

    /// The pool relics whose `IsAllowed` is overridden (`grep "override bool IsAllowed("
    /// Models.Relics/`, minus the Event-pool `MassiveScroll`, `SilverCrucible` and
    /// `WingedBoots`).
    const CULLED: [&str; 17] = [
        "AMETHYST_AUBERGINE",
        "BOOK_OF_FIVE_RINGS",
        "BOWLER_HAT",
        "DRAGON_FRUIT",
        "FROZEN_EGG",
        "GIRYA",
        "JUZU_BRACELET",
        "LASTING_CANDY",
        "LUCKY_FYSH",
        "MEAL_TICKET",
        "MOLTEN_EGG",
        "OLD_COIN",
        "PLANISPHERE",
        "SHOVEL",
        "TOXIC_EGG",
        "WHITE_BEAST_STATUE",
        "WHITE_STAR",
    ];

    #[test]
    fn every_culled_pool_relic_is_ported() {
        for id in CULLED {
            assert!(SHARED_RELIC_POOL.iter().any(|(r, _)| *r == id), "{id}");
            assert!(RelicKind::from_id(id).is_some(), "{id}");
        }
    }

    #[test]
    fn the_act3_treasure_floor_culls_on_the_next_pull() {
        let s = at_floor(40);
        assert!(CULLED.iter().all(|id| is_allowed(id, &s)));
        let s = at_floor(41);
        assert!(CULLED.iter().all(|id| !is_allowed(id, &s)));
        // `Remove` does not cull; the next pull culls every bucket, not only the one pulled.
        let mut bag = s.player_relic_bag.clone();
        let before: usize = bag.deques.iter().map(|(_, l)| l.len()).sum();
        bag.remove("ANCHOR");
        assert_eq!(
            bag.deques.iter().map(|(_, l)| l.len()).sum::<usize>(),
            before - 1
        );
        let pulled = bag.pull_from_front(Rare, &s).expect("a rare");
        assert!(!CULLED.contains(&pulled));
        let after: usize = bag.deques.iter().map(|(_, l)| l.len()).sum();
        assert_eq!(after, before - 2 - CULLED.len());
        assert!(
            bag.deques
                .iter()
                .all(|(_, l)| l.iter().all(|id| !CULLED.contains(id)))
        );
    }

    #[test]
    fn an_empty_shared_bucket_refills_in_pool_order_then_culls() {
        let mut s = at_floor(1);
        let mut bag = s.shared_relic_bag.clone();
        bag.deque_mut(Common).expect("commons").clear();
        // Refilled unshuffled, in `SharedRelicPool` order: `AmethystAubergine` first.
        assert_eq!(bag.pull_from_front(Common, &s), Some("AMETHYST_AUBERGINE"));
        let commons: Vec<_> = SHARED_RELIC_POOL
            .iter()
            .filter(|(_, r)| *r == Common)
            .map(|(id, _)| *id)
            .skip(1)
            .collect();
        assert_eq!(bag.deque(Common), Some(&commons));
        // Past the Act 3 treasure the refill is culled again: `Anchor` is the first allowed.
        s = at_floor(41);
        let mut bag = s.shared_relic_bag.clone();
        bag.deque_mut(Common).expect("commons").clear();
        assert_eq!(bag.pull_from_front(Common, &s), Some("ANCHOR"));
    }

    #[test]
    fn an_empty_player_bucket_falls_through_without_refilling() {
        let s = at_floor(1);
        let mut bag = s.player_relic_bag.clone();
        bag.deque_mut(Shop).expect("shop relics").clear();
        bag.deque_mut(Common).expect("commons").clear();
        let front = bag.deque(Uncommon).expect("uncommons")[0];
        assert_eq!(bag.pull_from_front(Shop, &s), Some(front));
        assert!(bag.deque(Shop).is_some_and(Vec::is_empty));
        assert!(bag.deque(Common).is_some_and(Vec::is_empty));
        // Past Rare there is nothing.
        bag.deques.iter_mut().for_each(|(_, l)| l.clear());
        assert_eq!(bag.pull_from_front(Common, &s), None);
        assert!(!bag.has_available_relics(&s));
    }

    #[test]
    fn a_bucket_the_bag_never_had_is_created_but_not_used_on_that_pull() {
        let s = at_floor(1);
        let mut bag = RelicGrabBag::populate(
            vec![("ANCHOR", Common), ("AKABEKO", Uncommon)],
            true,
            &mut Rng::new(1),
        );
        bag.deques.retain(|(r, _)| *r != Common);
        // `GetDeque(Common)` is a fresh list: the refill lands in a new bucket, the pull falls
        // through to Uncommon.
        assert_eq!(bag.pull_from_front(Common, &s), Some("AKABEKO"));
        assert_eq!(bag.deques, [(Uncommon, vec![]), (Common, vec!["ANCHOR"])]);
        assert_eq!(bag.pull_from_front(Common, &s), Some("ANCHOR"));
    }

    #[test]
    fn has_available_relics_sees_the_cull() {
        let s = at_floor(41);
        let mut bag = RelicGrabBag::populate(vec![("GIRYA", Rare)], false, &mut Rng::new(1));
        assert!(!bag.has_available_relics(&s));
        assert!(bag.has_available_relics(&at_floor(40)));
        assert_eq!(bag.pull_from_back(Rare, &s, |_| true), None);
    }
}
