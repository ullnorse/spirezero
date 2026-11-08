//! Rewards (`Rewards/*.cs`, `Factories/CardFactory.cs`, `Factories/RelicFactory.cs`): what a
//! won fight, a treasure chest or a relic offers, generated on the player's `rewards` stream.

use super::ascension::AscensionLevel;
use super::cards::{
    COLORLESS_CARD_POOL, PoolCard, RunCard, unlocked_cards, unlocked_colorless_cards,
};
use super::hooks;
use super::odds::{
    CardRarity, CardRarityOddsType, GoldProportion, REWARD_UPGRADE_BASE, RolledRelicRarity,
    UpgradeRoll, roll_for_upgrade,
};
use super::potions::random_potions;
use super::relic_bag::{Bag, RelicRarity};
use super::state::{Room, RoomType, RunState};
use crate::content::cards::CardType;
use crate::content::characters::Character;
use crate::content::encounters::Encounter;
use crate::rng::{PlayerRngType, Rng, RunRngType};

/// One reward on a rewards screen, already populated.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Reward {
    /// `GoldReward`.
    Gold(i32),
    /// `PotionReward`: the potion id.
    Potion(&'static str),
    /// `RelicReward`: the relic id.
    Relic(&'static str),
    /// `RelicReward(relic)` of a relic made wax (`IsWax`, `ToyBox`): claiming obtains it wax.
    WaxRelic(&'static str),
    /// `CardReward`: the cards offered; picking one adds it to the deck.
    Card(CardReward),
    /// `SpecialCardReward`: claiming it adds this card to the deck (`CardPileCmd.Add(card,
    /// Deck)`, `Rewards/SpecialCardReward.cs:86-96`).
    SpecialCard(RunCard),
}

/// A populated `CardReward` (`Rewards/CardReward.cs`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CardReward {
    /// The cards offered (`_cards`).
    pub cards: Vec<RunCard>,
    /// `Options`, which a relic obtained while the reward is up modifies the cards with
    /// (`OnRelicObtained`). `None` for a reward made from given cards (`Kaleidoscope`): that
    /// constructor does not listen to `RelicObtained`.
    pub options: Option<CardCreation>,
    /// `RerollOptions`: what a reroll generates with (the options themselves for a generated
    /// reward). `None` for options with no card pool (`Kaleidoscope`'s), which the engine does
    /// not reroll.
    pub reroll_options: Option<CardCreation>,
    /// `OptionCount`.
    pub count: usize,
    /// `CardCreationOptions.RngOverride` is the event's `Rng` (`CrystalSphereCardReward`): a
    /// reroll draws on it too.
    pub event_rng: bool,
    /// `CanReroll` (`Driftwood.TryModifyRewardsLate`); a reroll clears it.
    pub can_reroll: bool,
    /// An `AfterGenerated` listener upgrades every card the reward generates, a reroll's too
    /// (`TheFutureOfPotions.UpgradeCardsInReward`).
    pub upgrade_generated: bool,
}

impl CardReward {
    /// `new CardReward(options, count, player)` populated (`CardFactory.CreateForReward` with
    /// `options` plus `IsCardReward`).
    pub fn generate(s: &mut RunState, count: usize, options: CardCreation) -> Self {
        let options = options.as_card_reward();
        Self::with_cards(create_card_reward(s, count, options), options)
    }

    /// A reward of `cards` already created with `options` (`IsCardReward` included).
    #[must_use]
    pub const fn with_cards(cards: Vec<RunCard>, options: CardCreation) -> Self {
        Self {
            count: cards.len(),
            cards,
            options: Some(options),
            reroll_options: Some(options),
            event_rng: false,
            can_reroll: false,
            upgrade_generated: false,
        }
    }

    /// `new CardReward(cardsToOffer, source, player, rerollOptions)`: `reroll_options` `None`
    /// for options with no card pool.
    #[must_use]
    pub const fn of_cards(cards: Vec<RunCard>, reroll_options: Option<CardCreation>) -> Self {
        Self {
            count: cards.len(),
            cards,
            options: None,
            reroll_options,
            event_rng: false,
            can_reroll: false,
            upgrade_generated: false,
        }
    }
}

/// A card reward's alternatives ([`super::Run::card_reward_alternatives`]): `Skip`, maybe
/// `REROLL`, and what relics add; inline, as there are only a few.
pub type CardAlternatives = smallvec::SmallVec<[CardAlternative; 4]>;

/// One of a card reward's non-card options (`CardRewardAlternative.Generate`), in the game's
/// order: `Skip`, `REROLL`, then those `ModifyCardRewardAlternatives` adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardAlternative {
    /// `Skip` (`EndSelectionAndDoNotCompleteReward`): the card screen closes and the reward stays;
    /// the engine's [`super::RunAction::SkipReward`] covers it.
    Skip,
    /// `REROLL` (`DoNothing`): `CardReward.Reroll`, the screen stays up.
    Reroll,
    /// An alternative a relic adds (`TryModifyCardRewardAlternatives`): `PaelsWing`'s
    /// `SACRIFICE` (`EndSelectionAndCompleteReward`): the relic's callback, then the reward is
    /// done.
    Relic {
        /// The relic's index in the run's relics.
        relic: usize,
        /// The alternative's option id (`SACRIFICE`).
        option: &'static str,
    },
}

impl Reward {
    /// `RewardsSetIndex`: the screen's sort key.
    const fn sort_index(&self) -> u8 {
        match self {
            Self::Gold(_) => 1,
            Self::Potion(_) => 2,
            Self::Relic(_) | Self::WaxRelic(_) => 3,
            // `SpecialCardReward.cs:36`.
            Self::SpecialCard(_) => 4,
            Self::Card(_) => 5,
        }
    }
}

/// `CardReward`s `GoldReward(min, max)` bounds for a combat room (`EncounterModel.MinGoldReward`,
/// `MaxGoldReward`, each through [`poverty_gold`]; an encounter may override both), before a monster room's `GoldProportion`.
const fn gold_range(room: RoomType, ascension: u8) -> (i32, i32) {
    let (min, max) = match room {
        RoomType::Elite => (35, 45),
        RoomType::Boss => (100, 100),
        _ => (10, 20),
    };
    (poverty_gold(min, ascension), poverty_gold(max, ascension))
}

/// `AscensionLevel.Poverty`: gold × `PovertyAscensionGoldMultiplier` (0.75, a `double`), then
/// `(int)`, which truncates. The product of an `i32` and 0.75 is exact, so it is `3n / 4`
/// rounded toward zero.
#[must_use]
pub const fn poverty_gold(gold: i32, ascension: u8) -> i32 {
    if AscensionLevel::Poverty.is_active(ascension) {
        gold * 3 / 4
    } else {
        gold
    }
}

/// An unpopulated reward: the room's own rewards in generation order, or one a
/// `TryModifyRewards` listener adds (populated once every listener ran).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingReward {
    /// A reward populated at construction (`new GoldReward(amount, player)`).
    Ready(Reward),
    /// `GoldReward(min, max)`: `NextInt(min, max + 1)` on `rewards`.
    Gold(i32, i32),
    /// `PotionReward(player)`.
    Potion,
    /// `RelicReward(player)`: `PullNextRelicFromFront`.
    Relic,
    /// `RelicReward(rarity, player)`: `PullNextRelicFromFront(player, rarity)` (`CallingBell`).
    RelicOf(RelicRarity),
    /// `CardReward(options, count)`.
    Card(CardCreation, usize),
    /// [`PendingReward::Card`] with `CanReroll` set before it is populated (`Driftwood` on a
    /// reward another listener added).
    RerollableCard(CardCreation, usize),
}

impl PendingReward {
    /// `Reward.Populate`.
    pub(super) fn populate(self, s: &mut RunState) -> Reward {
        match self {
            Self::Ready(reward) => reward,
            Self::Gold(min, max) => {
                Reward::Gold(s.player_rng[PlayerRngType::Rewards].next_int_range(min, max + 1))
            }
            Self::Potion => Reward::Potion(potion(s)),
            Self::Relic => Reward::Relic(pull_relic_from_front(s)),
            Self::RelicOf(rarity) => Reward::Relic(pull_relic_of_rarity(s, rarity, |_| true)),
            Self::Card(options, count) => Reward::Card(CardReward::generate(s, count, options)),
            Self::RerollableCard(options, count) => {
                let mut reward = CardReward::generate(s, count, options);
                reward.can_reroll = true;
                Reward::Card(reward)
            }
        }
    }
}

/// The rewards of a won fight, plus `extra` (an event fight's `extraRewards`).
///
/// `RewardsSet.WithRewardsFromRoom(room)` then `GenerateWithoutOffering()`
/// (`Rewards/RewardsSet.cs:84-147,206-246`): the potion roll (`ShouldForcePotionReward` first),
/// then each reward populated in list order (gold, potion, cards, relic, extras), then
/// `ModifyRewards` and the added rewards populated, `AfterModifyingRewards`, the stable sort by
/// `RewardsSetIndex`, and `BeforeCombatRewardOffered`. The last act's boss gives nothing, not even
/// the extras (`RewardsSet.cs:87-90` returns before adding `ExtraRewards`).
///
/// `gold` is the fight's `CombatRoom.GoldProportion`: a monster room gives no gold at `0f` and
/// otherwise `GoldReward(round(min * p), round(max * p))` (`RewardsSet.cs:225-228`); elites and
/// bosses ignore it.
pub fn generate_combat_rewards(
    s: &mut RunState,
    room: RoomType,
    gold: GoldProportion,
    extra: Vec<PendingReward>,
) -> Vec<Reward> {
    let mut pending = Vec::new();
    if room != RoomType::Boss || s.act_index + 1 < s.acts.len() {
        let overridden = match s.room {
            Room::Combat { encounter, .. } => {
                Encounter::from_entry(encounter).and_then(Encounter::gold_reward)
            }
            _ => None,
        };
        let (min, max) = overridden.unwrap_or_else(|| gold_range(room, s.ascension));
        if room != RoomType::Monster {
            pending.push(PendingReward::Gold(min, max));
        } else if gold.gives_gold() {
            pending.push(PendingReward::Gold(gold.scale(min), gold.scale(max)));
        }
        if hooks::should_force_potion_reward(s, room)
            || s.potion_reward_odds
                .roll(room, &mut s.player_rng[PlayerRngType::Rewards])
        {
            pending.push(PendingReward::Potion);
        }
        let options = CardCreation::for_combat(CardRarityOddsType::for_room(room));
        pending.push(PendingReward::Card(options, 3));
        if room == RoomType::Elite {
            pending.push(PendingReward::Relic);
        }
        pending.extend(extra);
    }
    let mut rewards: Vec<Reward> = pending.into_iter().map(|p| p.populate(s)).collect();
    let (added, modifiers) = hooks::modify_rewards(s, Some(room), &mut rewards);
    for p in added {
        let reward = p.populate(s);
        rewards.push(reward);
    }
    hooks::after_modifying_rewards(s, &modifiers);
    rewards.sort_by_key(Reward::sort_index);
    hooks::before_combat_reward_offered(s, &mut rewards, room);
    rewards
}

/// `RewardsCmd.OfferCustom(player, rewards)` for populated `rewards`.
///
/// `Commands/RewardsCmd.cs:47-50`: a rewards set with no room, so `ModifyRewards` sees
/// `room == null`; then the added rewards and the sort by `RewardsSetIndex`.
pub fn custom_rewards(s: &mut RunState, mut rewards: Vec<Reward>) -> Vec<Reward> {
    let (added, modifiers) = hooks::modify_rewards(s, None, &mut rewards);
    for p in added {
        let reward = p.populate(s);
        rewards.push(reward);
    }
    hooks::after_modifying_rewards(s, &modifiers);
    rewards.sort_by_key(Reward::sort_index);
    rewards
}

/// `PotionReward.Populate`: one out-of-combat potion on `rewards`.
fn potion(s: &mut RunState) -> &'static str {
    random_potions(s.character, 1, &mut s.player_rng[PlayerRngType::Rewards])[0]
}

/// How a card reward picks rarities (`CardCreationOptions.RarityOdds` and `Source`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardOdds {
    /// Source `Encounter`: `CardRarityOdds.Roll`, which moves the pity offset.
    Encounter(CardRarityOddsType),
    /// Any other source: `RollWithBaseOdds`, no pity change.
    Base(CardRarityOddsType),
    /// `Uniform`: no rarity roll; any card but Basic and Ancient ones.
    Uniform,
}

/// The other characters' pools the `CharacterCards` modifiers union in.
///
/// In the order their modifiers act (`options.CardPools.Union([pool])` appends a pool not
/// already there). The custom screen has one modifier per character, so five at most, one of them the player's own
/// (a no-op).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ExtraPools([Option<Character>; 5]);

impl ExtraPools {
    /// These pools and `other` after them (unless it is already there).
    #[must_use]
    pub fn with(mut self, other: Character) -> Self {
        for slot in &mut self.0 {
            match *slot {
                Some(c) if c == other => break,
                Some(_) => {}
                None => {
                    *slot = Some(other);
                    break;
                }
            }
        }
        self
    }

    /// Just `other`.
    #[must_use]
    pub fn of(other: Character) -> Self {
        Self([None; 5]).with(other)
    }

    /// The characters, in order.
    pub fn iter(self) -> impl Iterator<Item = Character> {
        self.0.into_iter().flatten()
    }
}

/// The card pool a reward draws from (`CardCreationOptions.CardPools`, one pool).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardPool {
    /// `player.Character.CardPool`.
    Character,
    /// `ColorlessCardPool`.
    Colorless,
    /// `CardPools.Union([ColorlessCardPool])` on the character pool (`DingyRug`): the character
    /// cards, then the colorless ones.
    CharacterAndColorless,
    /// A given character's pool, whoever the player is (`ColorfulPhilosophers`). Only used with
    /// `NoCardPoolModifications`.
    Of(Character),
    /// `UnlockState.CharacterCardPools.Union(pools)` on a character pool (`PrismaticGem`): every
    /// character's pool in `ModelDb.AllCharacters` order.
    AllCharacters,
    /// [`CardPool::AllCharacters`], then the colorless pool.
    AllCharactersAndColorless,
    /// `CardPools.Union([character's pool])` on the player's pool (`CharacterCards`): the
    /// player's cards, then each given character's.
    CharacterAnd(ExtraPools),
    /// The player's cards, the colorless ones (`DingyRug`, which acts before the modifiers), then
    /// each given character's.
    CharacterAndColorlessAnd(ExtraPools),
}

impl CardPool {
    /// The pool after `options.CardPools.Union(new[] { ColorlessCardPool })`.
    #[must_use]
    pub const fn union_colorless(self) -> Self {
        match self {
            Self::Colorless => Self::Colorless,
            Self::Character | Self::CharacterAndColorless => Self::CharacterAndColorless,
            // Not reached: `DingyRug` acts before the modifiers, so the colorless pool is in
            // before any other character's.
            Self::CharacterAnd(others) | Self::CharacterAndColorlessAnd(others) => {
                Self::CharacterAndColorlessAnd(others)
            }
            Self::AllCharacters | Self::AllCharactersAndColorless => {
                Self::AllCharactersAndColorless
            }
            // No listener changes a `NoCardPoolModifications` pool.
            Self::Of(_) => self,
        }
    }

    /// The pool's cards in `GetPossibleCards` order, for a `character`'s player.
    fn cards(self, character: Character) -> Vec<&'static PoolCard> {
        match self {
            Self::Character => collect_sized(unlocked_cards(character)),
            Self::Of(other) => collect_sized(unlocked_cards(other)),
            Self::CharacterAnd(others) => {
                let mut cards = collect_sized(unlocked_cards(character));
                for other in others.iter() {
                    let have: Vec<&str> = cards.iter().map(|c| c.id).collect();
                    cards.extend(unlocked_cards(other).filter(|c| !have.contains(&c.id)));
                }
                cards
            }
            Self::CharacterAndColorlessAnd(others) => {
                let mut cards = Self::CharacterAndColorless.cards(character);
                for other in others.iter() {
                    let have: Vec<&str> = cards.iter().map(|c| c.id).collect();
                    cards.extend(unlocked_cards(other).filter(|c| !have.contains(&c.id)));
                }
                cards
            }
            Self::Colorless => collect_sized(unlocked_colorless_cards()),
            Self::CharacterAndColorless => {
                let mut cards = collect_sized(unlocked_cards(character));
                cards.reserve(COLORLESS_CARD_POOL.len());
                let character: Vec<&str> = cards.iter().map(|c| c.id).collect();
                cards.extend(unlocked_colorless_cards().filter(|c| !character.contains(&c.id)));
                cards
            }
            Self::AllCharacters => {
                let mut cards = Vec::with_capacity(ALL_POOL_CARDS);
                cards.extend(
                    Character::MODEL_DB_ORDER
                        .into_iter()
                        .flat_map(unlocked_cards),
                );
                cards
            }
            Self::AllCharactersAndColorless => {
                let mut cards = Self::AllCharacters.cards(character);
                cards.reserve(COLORLESS_CARD_POOL.len());
                cards.extend(unlocked_colorless_cards());
                cards
            }
        }
    }
}

/// Every character pool's cards.
const ALL_POOL_CARDS: usize = {
    let mut n = 0;
    let mut i = 0;
    while i < Character::ALL.len() {
        n += Character::ALL[i].card_pool().len();
        i += 1;
    }
    n
};

/// `iter` collected into a list allocated once at its upper size bound (a filtered pool:
/// collecting it as is grows the list five times or so).
fn collect_sized<T>(iter: impl Iterator<Item = T>) -> Vec<T> {
    let mut list = Vec::with_capacity(iter.size_hint().1.unwrap_or(0));
    list.extend(iter);
    list
}

/// `CardCreationOptions.CardPoolFilter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardFilter {
    /// Any card.
    Any,
    /// Cards of this rarity.
    Rarity(CardRarity),
    /// Cards of this rarity and type.
    RarityAndType(CardRarity, CardType),
    /// `c.Type == type` (`InfestedAutomaton` Study).
    Type(CardType),
    /// A character card whose `EnergyCost` is canonically 0 and not X (`InfestedAutomaton` Touch
    /// Core).
    ZeroCost,
}

impl CardFilter {
    fn allows(self, card: &PoolCard) -> bool {
        match self {
            Self::Any => true,
            Self::Rarity(r) => card.rarity == r,
            Self::RarityAndType(r, t) => card.rarity == r && card.card_type == t,
            Self::Type(t) => card.card_type == t,
            Self::ZeroCost => {
                Character::of_card(card.id).is_some_and(|c| c.zero_cost_cards().contains(&card.id))
            }
        }
    }
}

/// `CardCreationOptions` as far as the ported rewards use them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CardCreation {
    /// The pool drawn from.
    pub pool: CardPool,
    /// The rarity odds.
    pub odds: CardOdds,
    /// `CardPoolFilter`.
    pub filter: CardFilter,
    /// Not `NoUpgradeRoll`.
    pub upgrade_roll: bool,
    /// `IsCardReward` (a `CardReward`'s options; `SilverCrucible` acts only on these).
    pub is_card_reward: bool,
    /// The other `CardCreationFlags` the ported rewards use.
    pub flags: CreationFlags,
}

/// The `CardCreationFlags` besides `NoUpgradeRoll` ([`CardCreation::upgrade_roll`]) and
/// `IsCardReward` ([`CardCreation::is_card_reward`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CreationFlags(u8);

impl CreationFlags {
    /// No flag.
    pub const NONE: Self = Self(0);
    /// `IsFromCombat`: the card reward of a won fight (`LastingCandy` acts only on these).
    pub const IS_FROM_COMBAT: Self = Self(1);
    /// `NoCardPoolModifications`: `ModifyCardRewardCreationOptions` listeners leave the pool
    /// alone (`DingyRug`).
    pub const NO_CARD_POOL_MODIFICATIONS: Self = Self(2);
    /// `NoHookUpgrades`: the eggs do not upgrade.
    pub const NO_HOOK_UPGRADES: Self = Self(4);
    /// `NoModifyHooks`: `TryModifyCardRewardOptions` does not run on the result.
    pub const NO_MODIFY_HOOKS: Self = Self(8);
    /// `NoRarityModification`: `BigGameHunter` leaves the options alone.
    pub const NO_RARITY_MODIFICATION: Self = Self(16);

    /// `HasFlag`.
    #[must_use]
    pub const fn has(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }

    /// Both sets of flags.
    #[must_use]
    pub const fn with(self, flag: Self) -> Self {
        Self(self.0 | flag.0)
    }
}

impl CardCreation {
    /// `CardCreationOptions.ForRoom` for a combat room, as a `CardReward` (`PrayerWheel`,
    /// `WhiteStar`).
    #[must_use]
    pub const fn for_room(kind: CardRarityOddsType) -> Self {
        Self {
            pool: CardPool::Character,
            odds: CardOdds::Encounter(kind),
            filter: CardFilter::Any,
            upgrade_roll: true,
            is_card_reward: true,
            flags: CreationFlags::NONE,
        }
    }

    /// The room's own card reward (`RewardsSet.WithRewardsFromRoom`): `ForRoom` plus
    /// `IsFromCombat`.
    #[must_use]
    pub const fn for_combat(kind: CardRarityOddsType) -> Self {
        let mut options = Self::for_room(kind);
        options.flags = CreationFlags::IS_FROM_COMBAT;
        options
    }

    /// `ForNonCombatWithDefaultOdds(pools)`: `RegularEncounter` base odds, `NoUpgradeRoll`.
    #[must_use]
    pub const fn non_combat_default(pool: CardPool) -> Self {
        Self {
            pool,
            odds: CardOdds::Base(CardRarityOddsType::RegularEncounter),
            filter: CardFilter::Any,
            upgrade_roll: false,
            is_card_reward: false,
            flags: CreationFlags::NONE,
        }
    }

    /// `ForNonCombatWithUniformOdds(pools, filter)`: `Uniform`, `NoUpgradeRoll`.
    #[must_use]
    pub const fn non_combat_uniform(pool: CardPool, filter: CardFilter) -> Self {
        Self {
            pool,
            odds: CardOdds::Uniform,
            filter,
            upgrade_roll: false,
            is_card_reward: false,
            flags: CreationFlags::NONE,
        }
    }

    /// The same options with `flags` added (`WithFlags`).
    #[must_use]
    pub const fn with_flags(mut self, flags: CreationFlags) -> Self {
        self.flags = self.flags.with(flags);
        self
    }

    /// The same options with `NoCardPoolModifications`.
    #[must_use]
    pub const fn without_pool_modifications(mut self) -> Self {
        self.flags = self.flags.with(CreationFlags::NO_CARD_POOL_MODIFICATIONS);
        self
    }

    /// The same options with `IsCardReward` (`new CardReward(options, ...)` adds it).
    #[must_use]
    pub const fn as_card_reward(mut self) -> Self {
        self.is_card_reward = true;
        self
    }
}

/// `CardFactory.CreateForReward(player, count, options)`.
///
/// `CardFactory.cs:89-109,214-261`: per card `ModifyCardRewardCreationOptions`, the rarity, the
/// pick (`NextItem` among the not yet offered cards of that rarity) and, unless `NoUpgradeRoll`,
/// the upgrade roll; then `TryModifyCardRewardOptions` and `AfterModifyingCardRewardOptions`.
///
/// # Panics
/// If the filtered pool has no card to offer (the game throws).
pub fn create_card_reward(s: &mut RunState, count: usize, options: CardCreation) -> Vec<RunCard> {
    create_card_reward_on(s, count, options, None)
}

/// [`create_card_reward`] with `CardCreationOptions.WithRngOverride(rng)`: the picks and upgrade
/// rolls draw on `rng` instead of the player's `rewards` (a rarity roll stays on the player's
/// odds).
///
/// # Panics
/// As [`create_card_reward`].
pub fn create_card_reward_on(
    s: &mut RunState,
    count: usize,
    options: CardCreation,
    mut rng: Option<&mut Rng>,
) -> Vec<RunCard> {
    let mut cards: Vec<RunCard> = Vec::with_capacity(count);
    for _ in 0..count {
        let card_options = hooks::modify_card_reward_creation_options(s, options);
        let pool: Vec<&PoolCard> = card_options
            .pool
            .cards(s.character)
            .into_iter()
            .filter(|c| card_options.filter.allows(c))
            .filter(|c| cards.iter().all(|o| o.id != c.id))
            .collect();
        cards.push(pick_card_on(
            s,
            pool,
            card_options.odds,
            options.upgrade_roll,
            rng.as_deref_mut(),
        ));
    }
    if !options.flags.has(CreationFlags::NO_MODIFY_HOOKS) {
        hooks::modify_card_reward_options(s, &mut cards, options);
    }
    cards
}

/// The pick of `CardFactory.CreateForReward` (private) from `pool`.
///
/// The pool is already filtered and without the cards offered so far: the rarity is rolled per
/// `odds`, `NextItem` picks among the cards of that rarity, then the upgrade roll unless
/// `NoUpgradeRoll`.
///
/// # Panics
/// If the pool has no card to offer (the game throws).
pub fn pick_card(
    s: &mut RunState,
    pool: Vec<&PoolCard>,
    odds: CardOdds,
    upgrade_roll: bool,
) -> RunCard {
    pick_card_on(s, pool, odds, upgrade_roll, None)
}

/// [`pick_card`] whose pick and upgrade roll draw on `rng_override` when it is `Some`
/// (`CardCreationOptions.RngOverride`, `CardFactory.cs:100,235`).
fn pick_card_on(
    s: &mut RunState,
    pool: Vec<&PoolCard>,
    odds: CardOdds,
    upgrade_roll: bool,
    mut rng_override: Option<&mut Rng>,
) -> RunCard {
    let rng = &mut s.player_rng[PlayerRngType::Rewards];
    let items: Vec<&PoolCard> = match odds {
        CardOdds::Uniform => pool
            .into_iter()
            .filter(|c| !matches!(c.rarity, CardRarity::Basic | CardRarity::Ancient))
            .collect(),
        CardOdds::Encounter(kind) | CardOdds::Base(kind) => {
            let rolled = if let CardOdds::Encounter(_) = odds {
                s.card_rarity_odds.roll(kind, rng)
            } else {
                s.card_rarity_odds.roll_with_base_odds(kind, rng)
            };
            let rarity = next_allowed_rarity(rolled, |r| pool.iter().any(|c| c.rarity == r))
                .expect("the pool has a reward rarity");
            pool.into_iter().filter(|c| c.rarity == rarity).collect()
        }
    };
    let rng = rng_override.as_deref_mut().unwrap_or(rng);
    let picked = rng.next_item(&items).expect("a card to offer");
    let mut card = RunCard::new(picked.id);
    let roll = UpgradeRoll {
        base_chance: REWARD_UPGRADE_BASE,
        act_index: s.act_index,
        ascension: s.ascension,
        rarity: picked.rarity,
        upgradable: card.is_upgradable(),
    };
    let rng = match rng_override {
        Some(rng) => rng,
        None => &mut s.player_rng[PlayerRngType::Rewards],
    };
    if upgrade_roll && roll_for_upgrade(roll, rng) {
        card.upgrade_level += 1;
    }
    card
}

/// `CardCreationOptions.GetPossibleCards(player)` for a `character`'s player: the unlocked cards
/// of the options' pool that pass its filter, in pool order.
#[must_use]
pub fn possible_cards(character: Character, options: CardCreation) -> Vec<&'static PoolCard> {
    options
        .pool
        .cards(character)
        .into_iter()
        .filter(|c| options.filter.allows(c))
        .collect()
}

/// `CardFactory.GetNextAllowedRarity`: wrap upwards until an allowed rarity; `None` once it is
/// back where it started.
pub(super) fn next_allowed_rarity(
    rolled: CardRarity,
    allowed: impl Fn(CardRarity) -> bool,
) -> Option<CardRarity> {
    let mut rarity = rolled;
    while !allowed(rarity) {
        rarity = rarity.next_with_wrapping()?;
        if rarity == rolled {
            return None;
        }
    }
    Some(rarity)
}

/// `RelicFactory.RollRarity(rng)` as a bag rarity.
pub(super) fn roll_relic_rarity(rng: &mut Rng) -> RelicRarity {
    match super::odds::roll_relic_rarity(rng) {
        RolledRelicRarity::Common => RelicRarity::Common,
        RolledRelicRarity::Uncommon => RelicRarity::Uncommon,
        RolledRelicRarity::Rare => RelicRarity::Rare,
    }
}

/// `RelicFactory.FallbackRelic`.
pub(super) const FALLBACK_RELIC: &str = "CIRCLET";

/// `RelicFactory.PullNextRelicFromFront(player)`.
///
/// `RelicFactory.cs:13-41`: a rarity on `rewards`, the front of that bucket of the player's bag (`Circlet` if none), and the relic is
/// also removed from the shared bag.
pub fn pull_relic_from_front(s: &mut RunState) -> &'static str {
    let rarity = roll_relic_rarity(&mut s.player_rng[PlayerRngType::Rewards]);
    pull_relic_of_rarity(s, rarity, |_| true)
}

/// `RelicFactory.PullNextRelicFromFront(player, rng)`: [`pull_relic_from_front`] with the rarity
/// rolled on `rng` (a `RelicReward.SetRng` override).
pub fn pull_relic_from_front_on(s: &mut RunState, rng: &mut Rng) -> &'static str {
    let rarity = roll_relic_rarity(rng);
    pull_relic_of_rarity(s, rarity, |_| true)
}

/// `RelicFactory.PullNextRelicFromFront(player, rarity, filter)` (`RelicFactory.cs:45-50`).
///
/// The first relic passing `filter` from the front of the player's bag (`Circlet` if none), also
/// removed from the shared bag.
pub fn pull_relic_of_rarity(
    s: &mut RunState,
    rarity: RelicRarity,
    filter: impl Fn(&str) -> bool,
) -> &'static str {
    let relic = s
        .with_relic_bag(Bag::Player, |bag, s| {
            bag.pull_from_front_where(rarity, s, filter)
        })
        .unwrap_or(FALLBACK_RELIC);
    s.shared_relic_bag.remove(relic);
    relic
}

/// `TreasureRoomRelicSynchronizer.BeginRelicPicking` for one player.
///
/// `TreasureRoomRelicSynchronizer.cs:89-111`: a rarity on `TreasureRoomRelics`, then the front
/// of the shared bag, which loses it (`Circlet` if none). The player's bag loses it only when it
/// is obtained.
pub fn roll_treasure_relic(s: &mut RunState) -> &'static str {
    let rarity = roll_relic_rarity(&mut s.rng[RunRngType::TreasureRoomRelics]);
    s.with_relic_bag(Bag::Shared, |bag, s| bag.pull_from_front(rarity, s))
        .unwrap_or(FALLBACK_RELIC)
}
