//! Events: one file per event, each a unit struct implementing [`EventModel`] (`IsAllowed`,
//! `CalculateVars` with the initial options, and the option callbacks).
//!
//! To add an event: its file `<name>.rs` declaring `pub struct <Name>`; `build.rs` generates the
//! variant, `ALL`, `id` and `model` (see `docs/specs/content-api.md`, "Registration"). Also a
//! variant in [`EventData`] if it keeps state between pages. Events act on the run through
//! [`EventCtx`]; see `docs/specs/content-api.md`, "Events".

include!(concat!(env!("OUT_DIR"), "/content/events.rs"));

pub use crate::run::events::{EventAction, EventCtx, EventOption, ExitedCombat, Page};
pub use display::EventVar;

mod display;

use crate::content::cards::CardType;
use crate::decimal::Decimal;
use crate::rng::Rng;
use crate::run::{RunError, RunState};

/// An event's mutable vars and private fields beyond its options, as plain data.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EventData {
    /// The event keeps nothing.
    None,
    /// One rolled `DynamicVar` (`SunkenStatue`/`ThisOrThat` `Gold`, `UnrestSite` `Heal`).
    Amount(Decimal),
    /// `JungleMazeAdventure`'s `SoloGold` and `JoinForcesGold`.
    JungleMaze {
        /// `SoloGold`.
        solo_gold: Decimal,
        /// `JoinForcesGold`.
        join_gold: Decimal,
    },
    /// `TabletOfTruth`'s `DecipherCount` and `DecipherMaxHpLoss`.
    TabletOfTruth {
        /// `DecipherCount`.
        decipher_count: i32,
        /// `DecipherMaxHpLoss`.
        cost: i32,
    },
    /// `SlipperyBridge`'s `NumberOfHoldOns`, `RandomCardToLose` and `SkippedRemovals` (deck
    /// indices; the deck does not change while the event runs).
    SlipperyBridge {
        /// `NumberOfHoldOns`.
        hold_ons: i32,
        /// `RandomCardToLose` (a deck index).
        card: usize,
        /// `SkippedRemovals` (deck indices).
        skipped: Vec<usize>,
    },
    /// `SunkenTreasury`'s `SmallChestGold` and `LargeChestGold`.
    SunkenTreasury {
        /// `SmallChestGold`.
        small: Decimal,
        /// `LargeChestGold`.
        large: Decimal,
    },
    /// `PunchOff`, `TheLanternKey` (`EventLayoutType.Combat`): the run's `Niche` stream as it
    /// was before the layout fight was created.
    CombatLayout {
        /// The `Niche` stream before the layout fight was created.
        niche_before: Rng,
    },
    /// `TheFutureOfPotions.PotionToCardType`: the card type rolled per belt slot.
    FutureOfPotions {
        /// The belt slot and the card type rolled for it.
        types: Vec<(usize, CardType)>,
    },
    /// `ColossalFlower.NumberOfDigs`.
    ColossalFlower {
        /// `NumberOfDigs`.
        digs: usize,
    },
    /// `EndlessConveyor`: `_numOfGrabs`, `_lastDishId` and `_currentDish`.
    EndlessConveyor {
        /// `_numOfGrabs`.
        grabs: u32,
        /// `_lastDishId`.
        last: &'static str,
        /// `_currentDish`.
        current: &'static str,
    },
    /// `CrystalSphere`'s minigame.
    CrystalSphere(Box<crystal_sphere::Minigame>),
    /// A relic the event rolled (`WelcomeToWongos.FeaturedItem`).
    Relic(&'static str),
    /// `RelicTrader`'s `_ownedRelics` (relic indices) and `_newRelics`.
    RelicTrader {
        /// `_ownedRelics` (relic indices).
        owned: Vec<usize>,
        /// `_newRelics`.
        new: Vec<&'static str>,
    },
    /// `FakeMerchant.Inventory`'s relic entries in order: the relic (`None` once bought) and its
    /// `Cost`.
    FakeMerchant {
        /// Each relic (`None` once bought) and its `Cost`.
        relics: Vec<(Option<&'static str>, i32)>,
    },
    /// `TinkerTime.ChosenCardType`.
    TinkerTime(CardType),
    /// `Trial`: the case drawn (`MERCHANT`, `NOBLE`, `NONDESCRIPT`) and the verdict's option
    /// tag (`MERCHANT_GUILTY`), for the pages' `TrialStory` and `TrialResult`.
    Trial {
        /// The case drawn.
        case: Option<&'static str>,
        /// The verdict's option tag.
        verdict: Option<&'static str>,
    },
}

/// An event's `IsAllowed`, `CalculateVars`, initial options and option callbacks
/// (`EventModel`).
pub(crate) trait EventModel {
    /// `IsAllowed(runState)`, read on the canonical event (canonical vars) before it is picked.
    fn is_allowed(&self, _s: &RunState) -> bool {
        true
    }

    /// `BeforeEventStarted` sets `Owner.CanUseOrRemovePotions = false` and `OnEventFinished`
    /// sets it back: no potion is used or discarded outside combat while the event runs.
    fn locks_potions(&self) -> bool {
        false
    }

    /// `BeforeEventStarted`, `CalculateVars` and `GenerateInitialOptions`, in that order, on the
    /// event's own `Rng`; returns the first page's options.
    ///
    /// # Errors
    /// If the event meets content that is not ported.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError>;

    /// The callback of an option: `action` is what the option was built with.
    ///
    /// # Errors
    /// If the option needs content that is not ported; the run is then unchanged.
    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError>;

    /// The rest of a callback that asked for it ([`EventCtx::then`]) once its prompts are done
    /// (a deck selection, a rewards screen), with the page to show. The prompts are done even if
    /// the callback opened none.
    ///
    /// # Panics
    /// For an event that never calls [`EventCtx::then`].
    fn after_prompt(&self, action: EventAction, _ctx: &mut EventCtx<'_>) -> Page {
        unreachable!("{action:?} does not go on after a prompt")
    }

    /// `Resume(exitedRoom)`: the event comes back after a fight it started with `resume`
    /// ([`EventCtx::enter_combat`]), once the fight's rewards are left (at once for an encounter
    /// without rewards, `ShouldGiveRewards`). Returns the page to show instead of the one the
    /// option returned (`None` keeps it, as the base `Resume` does); may open one prompt.
    fn resume(&self, _exited: &ExitedCombat, _ctx: &mut EventCtx<'_>) -> Option<Page> {
        None
    }

    /// The rest of a callback after its deck selection
    /// ([`EventCtx::select_deck_cards`](crate::run::events::EventCtx::select_deck_cards)):
    /// `cards` are the picked deck indices in pick order. The page the callback returned is
    /// already up.
    ///
    /// # Errors
    /// If it meets content that is not ported; the selection is then rolled back.
    ///
    /// # Panics
    /// For an event that never selects deck cards.
    fn on_cards_selected(
        &self,
        action: EventAction,
        _ctx: &mut EventCtx<'_>,
        _cards: &[usize],
    ) -> Result<(), RunError> {
        unreachable!("{action:?} selects no deck cards")
    }
}

impl EventKind {
    /// `Player.CanUseOrRemovePotions` is false while the event runs
    /// ([`EventModel::locks_potions`]).
    #[must_use]
    pub fn locks_potions(self) -> bool {
        self.model().locks_potions()
    }
}
