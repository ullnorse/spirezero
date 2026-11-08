//! Run modifiers.
//!
//! Run modifiers (`Models.Modifiers/*.cs`, `Models/ModifierModel.cs`): what a custom run (or a
//! daily) adds to the run. One file per modifier holds its effect; this file is the list, the
//! lobby's rules (good and bad, mutually exclusive, `Pick2Good1Bad`) and the hook dispatch the
//! run layer calls, after the relics (`RunState.IterateHookListeners` lists the deck, the relics,
//! the potions, then the modifiers). The ones that act inside a combat set flags on the fight
//! ([`CombatModifiers`]).

mod all_star;
mod big_game_hunter;
mod character_cards;
mod cursed_run;
mod deadly_events;
pub mod draft;
mod flight;
mod hoarder;
mod insanity;
mod midas;
pub mod murderous;
mod night_terrors;
pub mod sealed_deck;
mod specialized;
pub mod terminal;
mod vintage;

#[cfg(test)]
mod tests;

use super::map::{ActMap, MapError};
use super::rewards::{CardCreation, PendingReward, Reward};
use super::{RestOption, RunCard, RunState};
use super::{RoomType, ascension::AscensionLevel};
use crate::content::characters::Character;
use crate::decimal::Decimal;
use crate::rng::Rng;

/// A `ModifierModel` of a custom or daily run (`DeprecatedModifier`, which only stands in for a
/// retired one in an old save, has no effect and is not listed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Modifier {
    /// `AllStar`.
    AllStar,
    /// `BigGameHunter`.
    BigGameHunter,
    /// `CharacterCards`, with its `CharacterModel`: that character's cards join the run's.
    CharacterCards(Character),
    /// `CursedRun`.
    CursedRun,
    /// `DeadlyEvents`.
    DeadlyEvents,
    /// `Draft`.
    Draft,
    /// `Flight`.
    Flight,
    /// `Hoarder`.
    Hoarder,
    /// `Insanity`.
    Insanity,
    /// `Midas`.
    Midas,
    /// `Murderous`.
    Murderous,
    /// `NightTerrors`.
    NightTerrors,
    /// `SealedDeck`.
    SealedDeck,
    /// `Specialized`.
    Specialized,
    /// `Terminal`.
    Terminal,
    /// `Vintage`.
    Vintage,
}

/// `ModelDb.GoodModifiers`, in the game's order (`CharacterCards` without a character).
const GOOD: [fn(Character) -> Modifier; 9] = [
    |_| Modifier::Draft,
    |_| Modifier::SealedDeck,
    |_| Modifier::Hoarder,
    |_| Modifier::Specialized,
    |_| Modifier::Insanity,
    |_| Modifier::AllStar,
    |_| Modifier::Flight,
    |_| Modifier::Vintage,
    Modifier::CharacterCards,
];

/// `ModelDb.BadModifiers`, in the game's order.
const BAD: [Modifier; 7] = [
    Modifier::DeadlyEvents,
    Modifier::CursedRun,
    Modifier::BigGameHunter,
    Modifier::Midas,
    Modifier::Murderous,
    Modifier::NightTerrors,
    Modifier::Terminal,
];

impl Modifier {
    /// The `ModelId.Entry` (`CharacterCards` shares its entry between characters).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::AllStar => "ALL_STAR",
            Self::BigGameHunter => "BIG_GAME_HUNTER",
            Self::CharacterCards(_) => "CHARACTER_CARDS",
            Self::CursedRun => "CURSED_RUN",
            Self::DeadlyEvents => "DEADLY_EVENTS",
            Self::Draft => "DRAFT",
            Self::Flight => "FLIGHT",
            Self::Hoarder => "HOARDER",
            Self::Insanity => "INSANITY",
            Self::Midas => "MIDAS",
            Self::Murderous => "MURDEROUS",
            Self::NightTerrors => "NIGHT_TERRORS",
            Self::SealedDeck => "SEALED_DECK",
            Self::Specialized => "SPECIALIZED",
            Self::Terminal => "TERMINAL",
            Self::Vintage => "VINTAGE",
        }
    }

    /// The `CharacterCards` character.
    #[must_use]
    pub const fn character(self) -> Option<Character> {
        match self {
            Self::CharacterCards(c) => Some(c),
            _ => None,
        }
    }

    /// The modifier in a save or on a page: the id, then `:Character` for `CharacterCards`.
    #[must_use]
    pub fn key(self) -> String {
        self.character().map_or_else(
            || self.id().to_owned(),
            |c| format!("{}:{}", self.id(), c.name()),
        )
    }

    /// [`Modifier::key`] read back.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        let (id, character) = match key.split_once(':') {
            Some((id, name)) => (id, Some(Character::from_name(name)?)),
            None => (key, None),
        };
        Self::all_ids().into_iter().find_map(|m| {
            (m.id() == id).then_some(match (m, character) {
                (Self::CharacterCards(_), Some(c)) => Self::CharacterCards(c),
                (Self::CharacterCards(_), None) => return None,
                (m, _) => m,
            })
        })
    }

    fn all_ids() -> Vec<Self> {
        let mut all: Vec<Self> = GOOD.iter().map(|m| m(Character::default())).collect();
        all.extend(BAD);
        all
    }

    /// The custom run screen's list (`NCustomRunModifiersList.GetAllModifiers`): the good
    /// modifiers, `CharacterCards` once per character (`ModelDb.AllCharacters`), then the bad
    /// ones.
    #[must_use]
    pub fn selectable() -> Vec<Self> {
        let mut list = Vec::new();
        for make in GOOD {
            if make(Character::default()).id() == "CHARACTER_CARDS" {
                list.extend(Character::MODEL_DB_ORDER.map(Self::CharacterCards));
            } else {
                list.push(make(Character::default()));
            }
        }
        list.extend(BAD);
        list
    }

    /// Whether it is among `ModelDb.GoodModifiers` (green on the screen; the bad ones are red).
    #[must_use]
    pub fn is_good(self) -> bool {
        !BAD.contains(&self)
    }

    /// `ModelDb.MutuallyExclusiveModifiers`: `SealedDeck`, `Draft` and `Insanity` each empty the
    /// deck and refill it, so at most one of them is ticked.
    #[must_use]
    pub const fn excludes(self, other: Self) -> bool {
        matches!(self, Self::SealedDeck | Self::Draft | Self::Insanity)
            && matches!(other, Self::SealedDeck | Self::Draft | Self::Insanity)
            && !matches!(
                (self, other),
                (Self::SealedDeck, Self::SealedDeck)
                    | (Self::Draft, Self::Draft)
                    | (Self::Insanity, Self::Insanity)
            )
    }

    /// `ClearsPlayerDeck`.
    #[must_use]
    pub const fn clears_deck(self) -> bool {
        matches!(self, Self::Draft | Self::Insanity | Self::SealedDeck)
    }

    /// Whether `GenerateNeowOption` gives an option.
    #[must_use]
    pub const fn has_neow_option(self) -> bool {
        matches!(
            self,
            Self::AllStar | Self::Draft | Self::Insanity | Self::SealedDeck | Self::Specialized
        )
    }

    /// The daily run for `date` (`dd_MM_yyyy`) played alone (`NDailyRunScreen.SetupLobbyParams`
    /// after the screen has settled).
    ///
    /// # Panics
    ///
    /// Never: the character list is not empty and the ascension is in `0..=10`.
    ///
    /// Three `NextUnsignedLong`-seeded streams of
    /// `new Rng(hash(canonical date))` pick the character (`NextItem(ModelDb.AllCharacters)`), the
    /// ascension (`NextInt(0, 11)`) and, with the character already in the lobby, `Pick2Good1Bad`.
    #[must_use]
    pub fn daily_run(date: &str) -> DailyRun {
        let canonical = crate::rng::canonicalize_seed(date);
        let mut rng = Rng::new(crate::rng::xxh64(&canonical));
        let mut characters = Rng::new(rng.next_unsigned_long());
        let mut ascensions = Rng::new(rng.next_unsigned_long());
        let mut modifiers = Rng::new(rng.next_unsigned_long());
        let character = *characters
            .next_item(&Character::MODEL_DB_ORDER)
            .expect("characters");
        DailyRun {
            character,
            ascension: u8::try_from(ascensions.next_int_range(0, 11)).expect("0..=10"),
            seed: crate::rng::canonicalize_seed(&format!("{date}_1p")),
            modifiers: Self::pick_2_good_1_bad(&mut modifiers, &[character]),
        }
    }

    /// `ModifierModel.Pick2Good1Bad`: two good modifiers, each removing its mutually exclusive
    /// ones from the choice (a `CharacterCards` takes a character not in `excluded`), then one
    /// bad one.
    ///
    /// # Panics
    /// Never: the lists are never exhausted.
    pub fn pick_2_good_1_bad(rng: &mut Rng, excluded: &[Character]) -> Vec<Self> {
        let characters: Vec<Character> = Character::MODEL_DB_ORDER
            .into_iter()
            .filter(|c| !excluded.contains(c))
            .collect();
        let mut good: Vec<fn(Character) -> Self> = GOOD.to_vec();
        if characters.is_empty() {
            good.retain(|m| m(Character::default()).id() != "CHARACTER_CARDS");
        }
        let mut picked = Vec::with_capacity(3);
        for _ in 0..2 {
            let make = *rng.next_item(&good).expect("enough good modifiers");
            let canonical = make(Character::default());
            let modifier = match canonical {
                Self::CharacterCards(_) => {
                    Self::CharacterCards(*rng.next_item(&characters).expect("a character"))
                }
                m => m,
            };
            picked.push(modifier);
            good.retain(|m| {
                let other = m(Character::default());
                other != canonical && !canonical.excludes(other)
            });
        }
        picked.push(*rng.next_item(&BAD).expect("a bad modifier"));
        picked
    }
}

/// A daily run's lobby: what the date decides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DailyRun {
    /// The hero.
    pub character: Character,
    /// The ascension level.
    pub ascension: u8,
    /// The run seed, canonical (`dd_MM_yyyy_1P`).
    pub seed: String,
    /// The two good modifiers and the bad one.
    pub modifiers: Vec<Modifier>,
}

/// What a fight takes from the run's modifiers (`CombatState` has the run's modifiers as
/// listeners).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CombatModifiers {
    /// `Murderous`: 3 Strength for every creature.
    pub murderous: bool,
    /// `Terminal`: 5 Plating for the player.
    pub terminal: bool,
}

impl CombatModifiers {
    /// The flags of a run's modifiers.
    #[must_use]
    pub fn of(modifiers: &[Modifier]) -> Self {
        Self {
            murderous: modifiers.contains(&Modifier::Murderous),
            terminal: modifiers.contains(&Modifier::Terminal),
        }
    }
}

/// What a modifier's Neow option does once chosen, beyond the cards it added at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NeowOutcome {
    /// Done: the cards joined the deck.
    Done,
    /// `Draft`: [`draft::PICKS`] card rewards follow.
    Draft,
    /// `SealedDeck`: pick [`sealed_deck::KEPT`] of these.
    Sealed(Vec<RunCard>),
}

/// `OnRunCreated` for every modifier, in order, after the relic bags are filled (`RunManager.
/// InitializeNewRun`): the deck cleared where the modifier says so (the ascension's card is
/// added after), then `AfterRunCreated`.
pub(super) fn on_run_created(s: &mut RunState) {
    for modifier in s.modifiers.clone() {
        if modifier.clears_deck() {
            s.deck.clear();
        }
        if modifier == Modifier::DeadlyEvents {
            deadly_events::after_run_created(s);
        }
    }
    if s.deck.is_empty()
        && s.modifiers.iter().any(|m| m.clears_deck())
        && AscensionLevel::AscendersBane.is_active(s.ascension)
    {
        s.deck.push(RunCard::new("ASCENDERS_BANE"));
    }
}

/// `AfterActEntered`.
pub(super) fn after_act_entered(s: &mut RunState) {
    for modifier in s.modifiers.clone() {
        if modifier == Modifier::CursedRun {
            cursed_run::after_act_entered(s);
        }
    }
}

/// `AfterRoomEntered` for `RunState.BaseRoom`, Neow's room.
pub(super) fn after_base_room_entered(s: &mut RunState) {
    if s.modifiers.contains(&Modifier::Terminal) {
        terminal::after_base_room_entered(s);
    }
}

/// `GenerateNeowOption` of each modifier, in order: the pages Neow shows one after another.
#[must_use]
pub(super) fn neow_options(s: &RunState) -> Vec<Modifier> {
    s.modifiers
        .iter()
        .copied()
        .filter(|m| m.has_neow_option())
        .collect()
}

/// The effect of `modifier`'s Neow option.
///
/// # Panics
/// For a modifier without an option.
pub(super) fn choose_neow_option(s: &mut RunState, modifier: Modifier) -> NeowOutcome {
    match modifier {
        Modifier::AllStar => all_star::obtain_cards(s),
        Modifier::Specialized => specialized::obtain_cards(s),
        Modifier::Insanity => insanity::obtain_cards(s),
        Modifier::Draft => return NeowOutcome::Draft,
        Modifier::SealedDeck => return NeowOutcome::Sealed(sealed_deck::offer(s)),
        _ => unreachable!("{modifier:?} has no Neow option"),
    }
    NeowOutcome::Done
}

/// One reward of the `Draft`.
pub(super) fn draft_reward(s: &mut RunState) -> Reward {
    draft::reward(s)
}

/// `Draft` and `SealedDeck` end by taking `PandorasBox` out of the bags (`Insanity` does so
/// with its cards).
pub(super) fn remove_pandoras_box(s: &mut RunState) {
    s.player_relic_bag.remove("PANDORAS_BOX");
    s.shared_relic_bag.remove("PANDORAS_BOX");
}

/// `ShouldAllowFreeTravel`.
pub(super) fn allows_free_travel(s: &RunState) -> bool {
    s.modifiers.contains(&Modifier::Flight) && flight::allows_free_travel()
}

/// `ShouldAllowMerchantCardRemoval`.
pub(super) fn allows_merchant_card_removal(s: &RunState) -> bool {
    !s.modifiers.contains(&Modifier::Hoarder)
}

/// `TryModifyRewardsLate`, after the relics'.
pub(super) fn modify_rewards_late(
    s: &mut RunState,
    room: Option<RoomType>,
    rewards: &mut [Reward],
    added: &mut [PendingReward],
) {
    for modifier in s.modifiers.clone() {
        match modifier {
            Modifier::Midas => midas::double_gold(rewards, added),
            Modifier::Vintage => vintage::relics_for_cards(s, room, rewards, added),
            _ => {}
        }
    }
}

/// `TryModifyRestSiteOptions`, after the relics'.
pub(super) fn modify_rest_site_options(s: &RunState, options: &mut Vec<RestOption>) {
    if s.modifiers.contains(&Modifier::Midas) {
        midas::remove_smith(options);
    }
}

/// `ModifyRestSiteHealAmount`, after the relics'.
pub(super) fn modify_rest_site_heal_amount(s: &RunState, amount: Decimal) -> Decimal {
    if s.modifiers.contains(&Modifier::NightTerrors) {
        night_terrors::heal_amount(s)
    } else {
        amount
    }
}

/// `AfterRestSiteHeal`, after the relics'.
pub(super) fn after_rest_site_heal(s: &mut RunState, is_mimicked: bool) {
    if s.modifiers.contains(&Modifier::NightTerrors) {
        night_terrors::after_heal(s, is_mimicked);
    }
}

/// `AfterCardChangedPiles` for a card that joined the deck, after the relics'.
pub(super) fn after_card_added_to_deck(s: &mut RunState, card: RunCard) {
    if s.modifiers.contains(&Modifier::Hoarder) {
        hoarder::after_card_added_to_deck(s, card);
    }
}

/// `ModifyCardRewardCreationOptions`, after the relics' (the normal pass).
pub(super) fn modify_card_reward_creation_options(
    s: &RunState,
    mut options: CardCreation,
) -> CardCreation {
    for modifier in &s.modifiers {
        options = match *modifier {
            Modifier::BigGameHunter => big_game_hunter::modify_options(s, options),
            Modifier::CharacterCards(other) => character_cards::modify_options(other, options),
            _ => options,
        };
    }
    options
}

/// `ModifyGeneratedMap`: `BigGameHunter` replaces a standard or Spoils map. Returns the new map, if any.
///
/// # Errors
/// Where the replacement map's generation throws.
pub(super) fn modify_generated_map(
    s: &RunState,
    map: &ActMap,
    act_index: usize,
) -> Result<Option<ActMap>, MapError> {
    if s.modifiers.contains(&Modifier::BigGameHunter) {
        return big_game_hunter::modify_map(s, map, act_index).map(Some);
    }
    Ok(None)
}

/// `ModifyMerchantCardPool` for the character-card slots: the first `CharacterCards` adds its
/// character's cards (`CharacterCards.cs:36-47`; the second sees a pool that already has
/// foreign cards).
pub(super) fn merchant_extra_character(s: &RunState) -> Option<Character> {
    s.modifiers.iter().find_map(|m| m.character())
}
