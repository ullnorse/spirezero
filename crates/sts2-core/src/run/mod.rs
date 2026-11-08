//! The run layer: everything between the seed and the end of the run that is not combat.
//!
//! A [`Run`] goes from a seed through run init (relic bags and every act's rooms rolled on
//! `UpFront`), each act's ancient (Neow in Act 1) and map, from one act to the next after its
//! boss, to [`RunDecision::Victory`] after the last act's. Monster rooms run an inner [`Combat`] whose
//! result (HP, potions, RNG streams) flows back into the [`RunState`], then offer their rewards.
//! Rest sites, treasure rooms, shops, `?` rolls and events are modeled; relics act through
//! their run-level hooks (`hooks`). Content that is not ported yet is explicit: an unported
//! encounter, ancient option, relic, potion or deck card is an error, and unported events are stubs
//! marked [`RunDecision::Unsupported`].

mod accepts;
pub mod ascension;
pub mod cards;
mod ctx;
pub mod events;
pub mod hooks;
pub mod map;
pub mod modifiers;
pub mod neow;
pub mod odds;
pub mod potions;
pub mod relic_bag;
pub mod relics;
pub mod rewards;
mod rollback;
pub mod rooms;
pub mod shop;
pub mod state;
pub mod unlock;

use std::fmt;

pub use cards::{DeckVar, RunCard};
pub use ctx::RunCtx;
pub use events::{EventAction, EventOption, EventRoom};
pub use map::{ActMap, MapCoord, MapError, MapPoint, MapPointType, NeighborIter, Neighbors};
pub use modifiers::Modifier;
pub use relics::RunRelic;
pub use rewards::{
    CardAlternative, CardAlternatives, CardCreation, CardFilter, CardOdds, CardPool, CardReward,
    CreationFlags, PendingReward, Reward,
};
pub use rooms::Act1;
pub use shop::{CardSlot, Shop, ShopCard, ShopEntry, ShopPotion, ShopRelic};
pub use state::{MapPointHistoryEntry, Room, RoomType, RunState};
pub use unlock::UnlockState;

use crate::combat::{
    self, BeltPotion, Combat, CombatConfig, DeckCard, Encounter, ExtraReward, PromptText, Status,
};
use crate::content::ancients::{AncientKind, AncientOption, AncientRoom};
use crate::content::characters::Character;
use crate::content::enchantments::EnchantmentKind;
use crate::content::events::EventKind;
use crate::content::potions::PotionUsage;
use crate::content::relics::{RelicData, RelicKind, girya, neows_bones, pumpkin_candle};
use crate::decimal::Decimal;
use crate::rng::{PlayerRngSet, PlayerRngType, RunRngSet, RunRngType, canonicalize_seed};
use ascension::{AscensionLevel, MAX_ASCENSION};
use ctx::{AfterPrompt, Prompt};
use modifiers::NeowOutcome;
use odds::{CardRarityOdds, PotionRewardOdds, UnknownMapPointOdds};
use relic_bag::RelicGrabBag;
use rollback::Journal;
use rooms::EncounterRoom;
use unlock::IdSet;

/// `AscensionLevel.AscendersBane`: the curse joins the starting deck
/// (`AscensionManager.ApplyEffectsTo`).
#[expect(
    clippy::as_conversions,
    reason = "the level's discriminant is its number"
)]
pub(crate) const ASCENDERS_BANE: u8 = AscensionLevel::AscendersBane as u8;
/// `Player.initialMaxPotionSlotCount`.
const STARTING_POTION_SLOTS: usize = 3;

/// The starting belt: `AscensionManager.ApplyEffectsTo` takes a slot from `TightBelt` on.
const fn starting_potion_slots(ascension: u8) -> usize {
    if AscensionLevel::TightBelt.is_active(ascension) {
        STARTING_POTION_SLOTS - 1
    } else {
        STARTING_POTION_SLOTS
    }
}
/// `TheArchitect`'s `ModelId.Entry`: the last act's victory room (`RunManager.EnterNextAct`).
pub const THE_ARCHITECT: &str = "THE_ARCHITECT";
/// `SmithRestSiteOption.SmithCount`.
const SMITH_COUNT: usize = 1;
/// `CookRestSiteOption._cardsToRemove`.
const COOK_COUNT: usize = 2;
/// `CookRestSiteOption._maxHpGain`.
const COOK_MAX_HP: i32 = 5;
/// `CardRewardAlternative.Generate` throws with more alternatives than this.
const MAX_CARD_ALTERNATIVES: usize = 2;

/// The map points the player can travel to ([`RunDecision::Map`]): at most a row.
pub type NextNodes = smallvec::SmallVec<[MapCoord; map::COLUMNS]>;

/// A player input outside or inside combat.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RunAction {
    /// Pick option `index` of the ancient's page (Neow's in Act 1).
    AncientOption(usize),
    /// Choose option `index` of the current event page.
    EventOption(usize),
    /// Travel to a child of the current map point.
    ChooseNode(MapCoord),
    /// An action in the current combat.
    Combat(combat::Action),
    /// Claim the gold, potion or relic reward at this index of the rewards screen.
    ClaimReward(usize),
    /// Take card `card` of the card reward at index `reward`.
    PickCard {
        /// The card reward's index on the rewards screen.
        reward: usize,
        /// The card's index in the reward.
        card: usize,
    },
    /// Skip the reward at this index.
    SkipReward(usize),
    /// Choose alternative `alternative` of [`Run::card_reward_alternatives`] for the card reward
    /// at index `reward` (`REROLL`, `SACRIFICE`; `Skip` is [`RunAction::SkipReward`]).
    CardAlternative {
        /// The card reward's index on the rewards screen.
        reward: usize,
        /// The index in [`Run::card_reward_alternatives`].
        alternative: usize,
    },
    /// Choose rest-site option `index`.
    RestOption(usize),
    /// Use the potion in belt slot `slot` outside combat (an `AnyTime` potion, on the player).
    UsePotion(usize),
    /// Discard the potion in belt slot `slot` (`PotionCmd.Discard`), in a combat's play phase or
    /// on any screen where the belt is shown. Not listed by [`Run::legal_actions`] (a policy
    /// never needs it); [`Run::discard_actions`] lists the ones that `apply` accepts.
    DiscardPotion(usize),
    /// Pick option `index` of a deck card selection.
    SelectCard(usize),
    /// Take option `index` back out of a deck card selection's picks (clicking a picked card in
    /// `NSimpleCardSelectScreen`). Listed after every other action, so saves made before it
    /// existed replay unchanged.
    DeselectCard(usize),
    /// Confirm a selection that takes fewer picks than its maximum (`GnarledHammer`).
    ConfirmSelect,
    /// Cancel a cancelable card selection (Smith), returning to the rest site.
    CancelSelect,
    /// Take the treasure room's relic.
    TakeRelic,
    /// Buy the shop's card at this index (character cards, then colorless).
    BuyCard(usize),
    /// Buy the shop's relic at this index.
    BuyRelic(usize),
    /// Buy the shop's potion at this index.
    BuyPotion(usize),
    /// Use the shop's card removal: a cancelable deck selection.
    RemoveCard,
    /// Take bundle `index` of a choose-a-bundle screen (`ScrollBoxes`).
    SelectBundle(usize),
    /// Leave the current room: a rewards screen (skipping what is left), a treasure room, a
    /// shop, or a stub room.
    Proceed,
}

/// A rest-site option (`RestSiteOption`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RestOptionKind {
    /// `HealRestSiteOption`.
    Heal,
    /// `SmithRestSiteOption`.
    Smith,
    /// `LiftRestSiteOption` (`Girya`).
    Lift,
    /// `DigRestSiteOption` (`Shovel`).
    Dig,
    /// `HatchRestSiteOption` (`ByrdonisEgg`).
    Hatch,
    /// `KindleRestSiteOption` (`PumpkinCandle`).
    Kindle,
    /// `CloneRestSiteOption` (`PaelsGrowth`).
    Clone,
    /// `CookRestSiteOption` (`MeatCleaver`).
    Cook,
}

impl RestOptionKind {
    /// `OptionId`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Heal => "HEAL",
            Self::Smith => "SMITH",
            Self::Lift => "LIFT",
            Self::Dig => "DIG",
            Self::Hatch => "HATCH",
            Self::Kindle => "KINDLE",
            Self::Clone => "CLONE",
            Self::Cook => "COOK",
        }
    }
}

impl RunState {
    /// The numbers a rest-site option's `rest_site_ui` description (`OPTION_<id>.description`)
    /// is filled with, by name, as its `RestSiteOption.Description` adds them: Rest's `Heal`
    /// (`HealVar.PreviewValue`, after `ModifyRestSiteHealAmount`), Smith's `Count`, Train's
    /// `LiftsLeft`, Kindle's `RekindleAmount`, Cook's `Cards` and `MaxHp`. Text values
    /// (`RelicName`, `EnchantmentName`, Rest's `ExtraText`) are the caller's.
    #[must_use]
    pub fn rest_option_vars(&self, kind: RestOptionKind) -> Vec<(&'static str, Decimal)> {
        let count = |n: usize| Decimal::from(i32::try_from(n).unwrap_or(i32::MAX));
        match kind {
            RestOptionKind::Heal => vec![("Heal", self.rest_site_heal_amount())],
            RestOptionKind::Smith => vec![("Count", count(SMITH_COUNT))],
            RestOptionKind::Lift => self
                .relics
                .iter()
                .position(|r| r.kind == RelicKind::Girya)
                .map(|me| vec![("LiftsLeft", Decimal::from(girya::lifts_left(self, me)))])
                .unwrap_or_default(),
            RestOptionKind::Kindle => vec![(
                "RekindleAmount",
                Decimal::from(pumpkin_candle::KINDLE_AMOUNT),
            )],
            RestOptionKind::Cook => vec![
                ("Cards", count(COOK_COUNT)),
                ("MaxHp", Decimal::from(COOK_MAX_HP)),
            ],
            RestOptionKind::Dig | RestOptionKind::Hatch | RestOptionKind::Clone => Vec::new(),
        }
    }
}

/// A rest-site option and whether it can be chosen (`IsEnabled`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RestOption {
    /// The option.
    pub kind: RestOptionKind,
    /// `IsEnabled`.
    pub enabled: bool,
}

/// What a deck card selection is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectPurpose {
    /// Upgrade the picked cards (`Pomander`).
    Upgrade,
    /// The rest site's Smith: upgrade, cancelable.
    Smith,
    /// The rest site's Cook (`CookRestSiteOption`): remove the picked cards, then
    /// `GainMaxHp(5)`; cancelable.
    Cook,
    /// Remove the picked cards, then take `damage` (`PreciseScissors`, `PrecariousShears`).
    Remove {
        /// The damage taken after the removal.
        damage: i32,
    },
    /// Add the picked cards of [`CardSelect::offered`] to the deck
    /// (`SelectCardsToAddToDeckFromGrid`).
    AddToDeck,
    /// The shop's card removal: pay `cost`, remove the picked card, count the removal;
    /// cancelable unless a relic buys it (`LordsParasol`).
    ShopRemoval {
        /// The price.
        cost: i32,
        /// The player may back out.
        cancelable: bool,
    },
    /// `CardCmd.Enchant` the picked cards (`FromDeckForEnchantment`).
    Enchant {
        /// The enchantment.
        kind: EnchantmentKind,
        /// Its amount.
        amount: i32,
    },
    /// As [`SelectPurpose::Enchant`] with `MinSelect` 0 and manual confirmation (`Kifuda`,
    /// `GnarledHammer`): the player picks up to `count` cards, then confirms.
    EnchantUpTo {
        /// The enchantment.
        kind: EnchantmentKind,
        /// Its amount.
        amount: i32,
    },
    /// Add a copy of the picked card to the deck (`DollysMirror`, `FromDeckGeneric`).
    Clone,
    /// The waiting event's `on_cards_selected` with the picked cards (its own use of a
    /// `FromDeck*` selection: removal, transform, a generic pick); `text` is its prompt.
    Event {
        /// The event's callback.
        action: EventAction,
        /// The prompt's text.
        text: PromptText,
    },
    /// `CardSelectCmd.FromChooseACardScreen(offered, canSkip: true)`: pick one of
    /// [`CardSelect::offered`] or skip; the pick joins the deck, then `extra` (`HeftyTablet`'s
    /// `Injury`, added either way).
    ChooseACard {
        /// A card added to the deck either way (`ModelId.Entry`).
        extra: Option<&'static str>,
    },
    /// `CardCmd.TransformToRandom` the picked cards on the run's `Niche` stream (`NewLeaf`).
    TransformRandom,
    /// The relic at this index of the relics' own use of a `FromDeck*` selection: its
    /// `RelicModel::on_cards_selected` gets the picks (`PaelsTooth`).
    Relic {
        /// The relic's index in the run's relics.
        relic: usize,
    },
    /// As [`SelectPurpose::Relic`] with `MinSelect` 0 and manual confirmation (`Claws`): the
    /// player picks up to `count` cards, then confirms.
    RelicUpTo {
        /// The relic's index in the run's relics.
        relic: usize,
    },
    /// Per picked card in pick order, `CreateRandomCardForTransform` on the run's `Niche` stream,
    /// `CardCmd.Upgrade` it, then `CardCmd.Transform` (`Astrolabe`).
    TransformRandomUpgraded,
    /// As [`SelectPurpose::AddToDeck`] with `MinSelect` 0 and manual confirmation
    /// (`FromSimpleGridForRewards`, `SeaGlass`).
    AddToDeckUpTo,
}

impl SelectPurpose {
    /// `CardSelectorPrefs.Cancelable`: Smith, Cook and the player's shop removal; a
    /// choose-a-card screen can be skipped the same way.
    #[must_use]
    pub const fn is_cancelable(self) -> bool {
        match self {
            Self::Smith | Self::Cook | Self::ChooseACard { .. } => true,
            Self::ShopRemoval { cancelable, .. } => cancelable,
            _ => false,
        }
    }

    /// Whether the options are [`CardSelect::offered`] cards rather than deck cards.
    #[must_use]
    pub const fn picks_offered(self) -> bool {
        matches!(
            self,
            Self::AddToDeck | Self::AddToDeckUpTo | Self::ChooseACard { .. }
        )
    }
}

/// A pending card selection: the player picks `count` of `options` one at a time.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CardSelect {
    /// What the picks are for.
    pub purpose: SelectPurpose,
    /// The selectable cards, in the order the game shows them: deck indices, or for
    /// [`SelectPurpose::AddToDeck`] and [`SelectPurpose::ChooseACard`] indices into `offered`.
    pub options: Vec<usize>,
    /// The cards a grid selection shows that are not in the deck; empty otherwise.
    pub offered: Vec<RunCard>,
    /// The number of cards to pick.
    pub count: usize,
    /// Indices into `options` picked so far.
    pub picked: Vec<usize>,
}

impl CardSelect {
    /// `CardSelectorPrefs.MinSelect`: how many picks the selection needs before it can be
    /// confirmed (a choose-a-card screen and an up-to selection take none).
    #[must_use]
    pub const fn min(&self) -> usize {
        match self.purpose {
            SelectPurpose::ChooseACard { .. }
            | SelectPurpose::EnchantUpTo { .. }
            | SelectPurpose::RelicUpTo { .. }
            | SelectPurpose::AddToDeckUpTo => 0,
            _ => self.count,
        }
    }

    /// `CardSelectorPrefs.RequireManualConfirmation` for a selection of exactly `count` cards:
    /// the picks are final only at the confirm button, so a pick can be taken back until then
    /// (`SealedDeck`). Other selections end with their last pick (`CookRestSiteOption` confirms in the game too, but saves made with it taking two picks replay as they were), so only a pick before the last can be taken back there.
    #[must_use]
    pub const fn manual_confirm(&self) -> bool {
        match self.purpose {
            SelectPurpose::AddToDeck => self.count == modifiers::sealed_deck::KEPT,
            _ => false,
        }
    }

    /// Whether `ConfirmSelect` ends the selection now.
    #[must_use]
    pub const fn can_confirm(&self) -> bool {
        self.picked.len() >= self.min() && (self.min() < self.count || self.manual_confirm())
    }

    /// What the selection asks (`CardSelectorPrefs.Prompt`) in `state`: the shared
    /// `card_selection` text of the command that opened it, or the relic's or event page's own
    /// `selectionScreenPrompt`.
    #[must_use]
    pub fn prompt(&self, state: &RunState) -> SelectPrompt {
        let relic_own = |relic: usize| {
            state
                .relics
                .get(relic)
                .map_or(SelectPrompt::Shared(PromptText::Own), |r| {
                    SelectPrompt::Relic(r.kind.id())
                })
        };
        match self.purpose {
            // SmithRestSiteOption, Pomander, YummyCookie: UpgradeSelectionPrompt.
            SelectPurpose::Upgrade | SelectPurpose::Smith => {
                SelectPrompt::Shared(PromptText::Upgrade)
            }
            // CookRestSiteOption, the shop (OneOffSynchronizer.DoMerchantCardRemoval), the
            // removal relics and PaelsTooth: RemoveSelectionPrompt.
            SelectPurpose::Cook
            | SelectPurpose::Remove { .. }
            | SelectPurpose::ShopRemoval { .. }
            | SelectPurpose::Relic { .. } => SelectPrompt::Shared(PromptText::Remove),
            SelectPurpose::Enchant { .. } | SelectPurpose::EnchantUpTo { .. } => {
                SelectPrompt::Shared(PromptText::Enchant)
            }
            // NewLeaf, Astrolabe: TransformSelectionPrompt.
            SelectPurpose::TransformRandom | SelectPurpose::TransformRandomUpgraded => {
                SelectPrompt::Shared(PromptText::Transform)
            }
            SelectPurpose::Event { text, .. } => SelectPrompt::Shared(text),
            SelectPurpose::ChooseACard { .. } => SelectPrompt::Shared(PromptText::ChooseACard),
            // Claws, DollysMirror, SeaGlass: the relic's own SelectionScreenPrompt.
            SelectPurpose::RelicUpTo { relic } => relic_own(relic),
            SelectPurpose::Clone => SelectPrompt::Relic("DOLLYS_MIRROR"),
            SelectPurpose::AddToDeckUpTo => SelectPrompt::Relic("SEA_GLASS"),
            // EventModel.SelectCardsToAddToDeckFromGrid: the event page's own prompt.
            SelectPurpose::AddToDeck if matches!(state.room, Room::Ancient { id: "NEOW" }) => {
                SelectPrompt::Modifier("SEALED_DECK")
            }
            SelectPurpose::AddToDeck => match state.room {
                Room::Event {
                    id: "ROOM_FULL_OF_CHEESE",
                } => SelectPrompt::EventPage("ROOM_FULL_OF_CHEESE.pages.GORGE"),
                Room::Event { id: "BRAIN_LEECH" } => {
                    SelectPrompt::EventPage("BRAIN_LEECH.pages.SHARE_KNOWLEDGE")
                }
                _ => SelectPrompt::Shared(PromptText::Own),
            },
        }
    }

    /// The cards shown, in display order.
    #[must_use]
    pub fn cards(&self, deck: &[RunCard]) -> Vec<RunCard> {
        let source = if self.purpose.picks_offered() {
            &self.offered
        } else {
            deck
        };
        self.options.iter().map(|&i| source[i]).collect()
    }
}

/// What a deck selection asks, as a localization lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectPrompt {
    /// A shared prompt (`card_selection.TO_*`, the choose-a-card banner).
    Shared(PromptText),
    /// `relics.<id>.selectionScreenPrompt`.
    Relic(&'static str),
    /// `events.<page>.selectionScreenPrompt`.
    EventPage(&'static str),
    /// `modifiers.<id>.selectionPrompt`.
    Modifier(&'static str),
}

/// What the run is waiting for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunDecision<'a> {
    /// The act's ancient (Neow in Act 1) and its options, in display order.
    Ancient(&'a AncientRoom),
    /// An event page.
    Event(&'a EventRoom),
    /// The map: the points the player can travel to.
    Map(NextNodes),
    /// A combat is in progress.
    Combat(&'a Combat),
    /// A rewards screen: the rewards not yet claimed or skipped.
    Rewards(&'a [Reward]),
    /// A deck card selection.
    CardSelect(&'a CardSelect),
    /// A choose-a-bundle screen (`CardSelectCmd.FromChooseABundleScreen`): one bundle's cards
    /// join the deck.
    Bundles(&'a [Vec<RunCard>]),
    /// A rest site's options (after one is used, the ones left, if a relic keeps them).
    RestSite(&'a [RestOption]),
    /// An opened treasure chest: its relic, if any is left.
    Treasure(Option<&'static str>),
    /// A merchant room's inventory (prices through [`shop::price`]).
    Shop(&'a Shop),
    /// A stub room: only [`RunAction::Proceed`].
    Unsupported(MapPointType),
    /// The fight met content the engine does not port ([`Status::Unsupported`]) and stopped
    /// there; the run takes no further action. [`Run::combat`] still shows the fight.
    Stopped(combat::Unsupported),
    /// The player died.
    GameOver,
    /// The run is won: the last act's last boss is beaten and its rewards are done, and
    /// `EnterNextAct` has entered `TheArchitect`'s event room (`AbstractRoom.IsVictoryRoom`). Its
    /// dialogue options only lead to `PROCEED` (`TheArchitect.WinRun` → `RunManager.WinRun`), so
    /// the engine ends the run here.
    Victory,
}

/// Why a run could not be created or an action was rejected. A rejected action changes nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunError {
    /// The run config asks for something the run layer does not model yet.
    UnsupportedConfig(&'static str),
    /// The game would throw while generating this seed's map.
    Map(MapError),
    /// The encounter (`ModelId.Entry`) is not ported.
    UnsupportedEncounter(&'static str),
    /// The ancient option's relic (`ModelId.Entry`) has effects that are not ported.
    UnsupportedAncientOption(&'static str),
    /// The relic would be obtained, or would act in this room, but its effects are not ported.
    UnsupportedRelic(&'static str),
    /// The deck holds a card the combat engine does not port, so no fight can start.
    UnsupportedCard(&'static str),
    /// The belt holds a potion that acts in combat by itself and is not ported.
    UnsupportedPotion(&'static str),
    /// The combat rejected the action.
    Combat(combat::ActionError),
    /// The action does not answer the pending decision.
    IllegalAction,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedConfig(what) => write!(f, "unsupported run config: {what}"),
            Self::Map(e) => write!(f, "map generation failed: {e}"),
            Self::UnsupportedEncounter(id) => write!(f, "encounter {id} is not ported"),
            Self::UnsupportedAncientOption(id) => write!(f, "ancient option {id} is not ported"),
            Self::UnsupportedRelic(id) => write!(f, "relic {id} is not ported"),
            Self::UnsupportedCard(id) => write!(f, "card {id} is not ported"),
            Self::UnsupportedPotion(id) => write!(f, "potion {id} is not ported"),
            Self::Combat(e) => write!(f, "combat: {e}"),
            Self::IllegalAction => f.write_str("the action does not answer the pending decision"),
        }
    }
}

impl std::error::Error for RunError {}

impl From<combat::ActionError> for RunError {
    fn from(e: combat::ActionError) -> Self {
        Self::Combat(e)
    }
}

/// A rest site's state: its options and whether one was used (then it can be left).
#[derive(Clone, Debug, PartialEq, Eq)]
struct RestSite {
    options: Vec<RestOption>,
    used: bool,
    /// The option waiting for its prompt (Smith's selection, a heal's rewards) to finish: it is
    /// removed then.
    pending: Option<RestOptionKind>,
}

/// Where the run is between decisions.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    Ancient(AncientRoom),
    Event(Box<EventRoom>),
    Map,
    Combat(Box<Combat>),
    Rewards(Vec<Reward>),
    CardSelect(CardSelect),
    Bundles(Vec<Vec<RunCard>>),
    RestSite(RestSite),
    Treasure(Option<&'static str>),
    Shop(Box<Shop>),
    Unsupported(MapPointType),
    GameOver,
    Victory,
}

/// A run in progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    state: RunState,
    phase: Phase,
    /// Whether combats record events ([`Run::with_events`]).
    record_events: bool,
    /// Whether the run keeps a copy of the streams each fight starts with
    /// ([`Run::with_combat_start_streams`]).
    record_start_streams: bool,
    /// With [`Run::with_combat_start_streams`], the streams the last fight was set up with.
    combat_start_streams: Option<Box<RunRngSet>>,
    /// The event waiting for its prompt (a rewards screen, a card selection or a fight it
    /// resumes after) to finish, or the one the player died in.
    suspended_event: Option<Box<EventRoom>>,
    /// The last event finished and left, until the next room is entered.
    finished_event: Option<Box<EventRoom>>,
    /// With events recorded, the fight the last action ended (a play, or the room entry itself
    /// when the fight is over before the first play), until the next action ([`Run::ended_combat`]).
    ended_combat: Option<Box<Combat>>,
    /// The rest site waiting for its Smith selection.
    suspended_rest: Option<RestSite>,
    /// The shop waiting for its card removal or a bought relic's prompt.
    suspended_shop: Option<Box<Shop>>,
    /// The bought relic slot (and the gold spent) whose purchase finishes when the relic's
    /// prompt ends (`RelicCmd.Obtain` awaits `AfterObtained`).
    relic_purchase_after_prompt: Option<(usize, i32)>,
    /// What runs when the prompt a relic opened ends.
    after_prompt: Option<AfterPrompt>,
    /// The rewards screens and opened chests a relic claimed from them prompts over
    /// (`RelicReward.OnSelect` awaits `RelicCmd.Obtain`, whose `AfterObtained` opens its prompt
    /// on top), innermost last, each with the `after_prompt` it waits on; one comes back when
    /// the prompt over it ends.
    suspended_screens: Vec<(Phase, Option<AfterPrompt>)>,
    /// The current fight's `extraRewards` and whether it was started by an event.
    combat_extra_rewards: Vec<PendingReward>,
    /// The last fight's `CombatRoom.GoldProportion`.
    combat_gold: odds::GoldProportion,
    /// What a rejected action restores the run from (`rollback`).
    journal: Journal,
    /// The modifiers whose Neow options are still to come, the page showing first
    /// (`Neow.ModifierOptions`).
    neow_modifiers: Vec<Modifier>,
    /// The `Draft` card rewards still to offer, the one on screen included.
    draft_left: u8,
    /// The `SealedDeck` selection is up: its end finishes Neow's page.
    sealed_pending: bool,
}

impl Run {
    /// Starts a run as `character` on `seed` (any spelling; canonicalized like the game's) and
    /// plays it to the Neow event. `act1` is the lobby's `Act1` setting: [`Act1::Random`] (the
    /// default, `Act1::default()`, and what a player or a library user wants) rolls Act 1 as the
    /// real game does ([`rooms::select_acts`]), and `state.acts[0].act.id` names the roll; the
    /// referee never rolls (it always plays Overgrowth unless forced), so differential harnesses
    /// pass the act explicitly.
    ///
    /// # Errors
    /// [`RunError::UnsupportedConfig`] for an ascension above 10
    /// (`AscensionManager.maxAscensionAllowed`) or an unlock state other than every epoch
    /// revealed on a non-first run; [`RunError::Map`] where the game would throw.
    pub fn new(
        character: Character,
        seed: &str,
        ascension: u8,
        unlock: UnlockState,
        act1: Act1,
    ) -> Result<Self, RunError> {
        Self::new_with_modifiers(character, seed, ascension, unlock, act1, &[])
    }

    /// [`Run::new`] with the custom run's `modifiers` (`RunState.Modifiers`, in the order given).
    /// `OnRunCreated` runs after the relic bags are filled (the decks `Draft`, `Insanity` and
    /// `SealedDeck` empty are cleared, the ascension's card comes back), the first act is entered
    /// (`AfterActEntered`, the map through `ModifyGeneratedMap`), then Neow's room.
    ///
    /// # Errors
    /// As [`Run::new`].
    pub fn new_with_modifiers(
        character: Character,
        seed: &str,
        ascension: u8,
        unlock: UnlockState,
        act1: Act1,
        modifiers: &[Modifier],
    ) -> Result<Self, RunError> {
        if ascension > MAX_ASCENSION {
            return Err(RunError::UnsupportedConfig("ascension above 10"));
        }
        if unlock.is_first_run() || unlock.epochs_revealed != IdSet::All {
            return Err(RunError::UnsupportedConfig(
                "unlock state other than every epoch revealed on a non-first run",
            ));
        }
        // `RunState.CreateForNewRun`: streams created, no draws.
        let mut rng = RunRngSet::new(&canonicalize_seed(seed));
        let player_rng = PlayerRngSet::for_slot(rng.seed(), 0);
        // `InitializeNewRun`: the shared bag, then the player's, both on `UpFront`.
        let shared_relic_bag = RelicGrabBag::shared(&mut rng[RunRngType::UpFront]);
        let player_relic_bag = RelicGrabBag::player(character, &mut rng[RunRngType::UpFront]);
        // `StartRunLobby.BeginRunLocally`: the acts come from their own `act_selection` stream.
        let act_list = rooms::select_acts(rng.seed(), &unlock, act1);
        let room_sets = rooms::generate_rooms(&act_list, &mut rng[RunRngType::UpFront], ascension);
        let deck = starting_deck(character, ascension);
        // `FinalizeStartingRelics`: no starter relic has an `AfterObtained`.
        // `EnterAct(0)`: the map comes from its own `act_1_map` stream.
        let map = ActMap::generate_act1(&rng, ascension).map_err(RunError::Map)?;
        let state = RunState {
            character,
            ascension,
            test_mode: false,
            unlock,
            rng,
            player_rng,
            card_rarity_odds: CardRarityOdds::new(ascension),
            potion_reward_odds: PotionRewardOdds::default(),
            unknown_odds: UnknownMapPointOdds::default(),
            act_index: 0,
            acts: room_sets,
            map,
            visited: Vec::new(),
            history: Vec::new(),
            act_floor: 0,
            visited_event_ids: Vec::new(),
            room: Room::Map,
            hp: character.starting_hp(),
            max_hp: character.starting_hp(),
            gold: character.starting_gold(),
            max_energy: character.max_energy(),
            deck,
            relics: vec![RunRelic::new(character.starting_relic())],
            potions: vec![None; starting_potion_slots(ascension)],
            shared_relic_bag,
            player_relic_bag,
            card_shop_removals_used: 0,
            spoils_quest: None,
            modifiers: modifiers.to_vec(),
        };
        let mut state = state;
        if !modifiers.is_empty() {
            modifiers::on_run_created(&mut state);
            state.generate_map().map_err(RunError::Map)?;
            modifiers::after_act_entered(&mut state);
        }
        let start = state.map.start().coord;
        let mut run = Self {
            state,
            phase: Phase::Map,
            record_events: false,
            record_start_streams: false,
            combat_start_streams: None,
            suspended_event: None,
            finished_event: None,
            ended_combat: None,
            suspended_rest: None,
            suspended_shop: None,
            relic_purchase_after_prompt: None,
            after_prompt: None,
            suspended_screens: Vec::new(),
            combat_extra_rewards: Vec::new(),
            combat_gold: odds::GoldProportion::ONE,
            journal: Journal::default(),
            neow_modifiers: Vec::new(),
            draft_left: 0,
            sealed_pending: false,
        };
        run.enter_neow(start);
        Ok(run)
    }

    /// The same, with combats recording events ([`Combat::events`]).
    #[must_use]
    pub fn with_events(mut self) -> Self {
        self.record_events = true;
        self.forget_journal();
        self
    }

    /// The same, keeping a copy of the run's streams as each fight's set-up receives them
    /// ([`Run::combat_start_streams`]), so a fight's start can be written down and rebuilt.
    #[must_use]
    pub fn with_combat_start_streams(mut self) -> Self {
        self.record_start_streams = true;
        self.forget_journal();
        self
    }

    /// The same, with `TestMode.IsOn` (as the referee runs the game).
    #[must_use]
    pub fn in_test_mode(mut self) -> Self {
        self.state.test_mode = true;
        self.forget_journal();
        self
    }

    /// A debug hook for differential loadouts: the state to hand the player cards, gold or HP,
    /// while the run waits on the map. The referee's `run_loadout` does the same to the game.
    ///
    /// # Errors
    /// [`RunError::IllegalAction`] unless the run is on the map.
    pub fn loadout_state(&mut self) -> Result<&mut RunState, RunError> {
        if self.phase != Phase::Map {
            return Err(RunError::IllegalAction);
        }
        self.forget_journal();
        Ok(&mut self.state)
    }

    /// A debug hook for differential loadouts: `RelicCmd.Obtain` of `id` while the run waits on
    /// the map, so its `AfterObtained` runs. A relic that prompts leaves the run in that prompt;
    /// the map comes back when it ends.
    ///
    /// # Errors
    /// [`RunError::IllegalAction`] unless the run is on the map; [`RunError::UnsupportedRelic`]
    /// for a relic that is not ported.
    pub fn loadout_relic(&mut self, id: &'static str) -> Result<(), RunError> {
        if self.phase != Phase::Map {
            return Err(RunError::IllegalAction);
        }
        self.forget_journal();
        if let Some(prompt) = self.obtain_relic(id)? {
            self.phase = prompt;
        }
        Ok(())
    }

    /// The run state. During a combat, HP, potions and the streams are as of combat start (the
    /// combat owns the streams, see [`Run::rng`]); see [`Run::combat`] for the live fight and
    /// [`Run::potions`] for the live belt.
    #[must_use]
    pub const fn state(&self) -> &RunState {
        &self.state
    }

    /// The run's RNG streams, wherever they are: the combat's during a fight.
    #[must_use]
    pub fn rng(&self) -> &RunRngSet {
        self.combat().map_or(&self.state.rng, |c| &c.state().rng)
    }

    /// The combat in progress, if any.
    #[must_use]
    pub fn combat(&self) -> Option<&Combat> {
        match &self.phase {
            Phase::Combat(combat) => Some(combat),
            _ => None,
        }
    }

    /// During a fight, the run's streams as the fight's [`CombatConfig`] received them;
    /// `None` outside a fight or without [`Run::with_combat_start_streams`].
    #[must_use]
    pub fn combat_start_streams(&self) -> Option<&RunRngSet> {
        self.combat().and(self.combat_start_streams.as_deref())
    }

    /// The fight the last action ended, as it ended: kept only in a run with events recorded
    /// ([`Run::with_events`]), for traces that report a finished fight. A fight can end on the
    /// action that enters its room (`FurCoat`, `FestivePopper`), so [`Run::combat`] never shows it.
    #[must_use]
    pub fn ended_combat(&self) -> Option<&Combat> {
        self.ended_combat.as_deref()
    }

    /// The event the player is in, if any: on its page, behind its prompt, or died in.
    #[must_use]
    pub fn event(&self) -> Option<&EventRoom> {
        match &self.phase {
            Phase::Event(room) => Some(room),
            _ => self.suspended_event.as_deref(),
        }
    }

    /// The finished event whose last page ([`EventRoom::finished`]) is still to be read: the one
    /// behind the prompt its last callback opened, else the one just left, until the next room
    /// is entered.
    #[must_use]
    pub fn finished_event(&self) -> Option<&EventRoom> {
        self.suspended_event
            .as_deref()
            .filter(|e| e.finished.is_some())
            .or(self.finished_event.as_deref())
    }

    /// The potion belt by slot, live during a combat (the combat belt has the run's slots).
    #[must_use]
    pub fn potions(&self) -> Vec<Option<&'static str>> {
        match &self.phase {
            Phase::Combat(combat) => combat_belt(combat),
            _ => self.state.potions.clone(),
        }
    }

    /// What the run is waiting for.
    #[must_use]
    pub fn pending_decision(&self) -> RunDecision<'_> {
        match &self.phase {
            Phase::Ancient(room) => RunDecision::Ancient(room),
            Phase::Event(room) => RunDecision::Event(room),
            Phase::Map => RunDecision::Map(self.next_nodes()),
            Phase::Combat(combat) => match combat.status() {
                Status::Unsupported(unsupported) => RunDecision::Stopped(unsupported),
                _ => RunDecision::Combat(combat),
            },
            Phase::Rewards(rewards) => RunDecision::Rewards(rewards),
            Phase::CardSelect(select) => RunDecision::CardSelect(select),
            Phase::Bundles(bundles) => RunDecision::Bundles(bundles),
            Phase::RestSite(rest) => RunDecision::RestSite(&rest.options),
            Phase::Treasure(relic) => RunDecision::Treasure(*relic),
            Phase::Shop(shop) => RunDecision::Shop(shop),
            Phase::Unsupported(point) => RunDecision::Unsupported(*point),
            Phase::GameOver => RunDecision::GameOver,
            Phase::Victory => RunDecision::Victory,
        }
    }

    /// The relic of each option of the pending ancient (tests).
    #[cfg(test)]
    fn ancient_relics(&self) -> Vec<&'static str> {
        match &self.phase {
            Phase::Ancient(room) => room
                .options
                .iter()
                .map(|o| o.relic_id().expect("a relic option"))
                .collect(),
            _ => panic!("not on an ancient's page: {:?}", self.pending_decision()),
        }
    }

    /// The [`RunAction::DiscardPotion`]s that [`Run::apply`] accepts now: one per held potion,
    /// except during a card choice, a bundle choice or after the run ends.
    #[must_use]
    pub fn discard_actions(&self) -> Vec<RunAction> {
        let slots: Vec<usize> = match &self.phase {
            Phase::Combat(combat) => combat.discardable_potions(),
            _ if self.potions_locked() => Vec::new(),
            Phase::Ancient(_)
            | Phase::Event(_)
            | Phase::Map
            | Phase::Rewards(_)
            | Phase::RestSite(_)
            | Phase::Treasure(_)
            | Phase::Shop(_) => self.state.potions_held().map(|(slot, _)| slot).collect(),
            _ => Vec::new(),
        };
        slots.into_iter().map(RunAction::DiscardPotion).collect()
    }

    /// The actions that put back a card picked in the combat's pending card choice, in pick
    /// order. [`Run::apply`] accepts them, [`Run::legal_actions`] does not list them (a bot
    /// never needs them).
    #[must_use]
    pub fn unpick_actions(&self) -> Vec<RunAction> {
        let Phase::Combat(combat) = &self.phase else {
            return Vec::new();
        };
        combat.pending_decision().map_or_else(Vec::new, |request| {
            request
                .unpicks()
                .into_iter()
                .map(|d| RunAction::Combat(combat::Action::Resolve(d)))
                .collect()
        })
    }

    fn discard_potion(&mut self, slot: usize) -> Result<(), RunError> {
        if !self
            .discard_actions()
            .contains(&RunAction::DiscardPotion(slot))
        {
            return Err(RunError::IllegalAction);
        }
        if let Phase::Combat(combat) = &mut self.phase {
            combat.discard_potion(slot)?;
        } else {
            self.state.discard_potion(slot);
        }
        Ok(())
    }

    /// Every action that answers the pending decision. Some may still be rejected with an
    /// `Unsupported*` error because their content is not ported ([`Run::accepts`] tells which).
    #[must_use]
    pub fn legal_actions(&self) -> Vec<RunAction> {
        let mut actions = Vec::new();
        self.legal_actions_into(&mut actions);
        actions
    }

    /// [`Run::legal_actions`] into `actions` (cleared first), reusing its allocation.
    pub fn legal_actions_into(&self, actions: &mut Vec<RunAction>) {
        actions.clear();
        match &self.phase {
            Phase::Ancient(room) => {
                actions.extend(
                    (0..room.options.len())
                        .filter(|&i| !matches!(room.options[i], AncientOption::Locked(_)))
                        .map(RunAction::AncientOption),
                );
                self.push_potion_actions(actions);
            }
            Phase::Event(room) => {
                actions.extend(
                    (0..room.options.len())
                        .filter(|&i| room.options[i].action.is_some())
                        .map(RunAction::EventOption),
                );
                self.push_potion_actions(actions);
            }
            Phase::Map => {
                actions.extend(self.next_nodes().into_iter().map(RunAction::ChooseNode));
                self.push_potion_actions(actions);
            }
            Phase::Combat(combat) => {
                combat.for_each_legal_action(|action| actions.push(RunAction::Combat(action)));
            }
            Phase::Rewards(rewards) => self.push_reward_actions(rewards, actions),
            Phase::CardSelect(select) => Self::push_select_actions(select, actions),
            Phase::Bundles(bundles) => {
                actions.extend((0..bundles.len()).map(RunAction::SelectBundle));
            }
            Phase::RestSite(rest) => {
                actions.extend(
                    (0..rest.options.len())
                        .filter(|&i| rest.options[i].enabled)
                        .map(RunAction::RestOption),
                );
                if rest.used {
                    actions.push(RunAction::Proceed);
                }
                self.push_potion_actions(actions);
            }
            Phase::Treasure(relic) => {
                if relic.is_some() {
                    actions.push(RunAction::TakeRelic);
                }
                actions.push(RunAction::Proceed);
                self.push_potion_actions(actions);
            }
            Phase::Shop(shop) => self.push_shop_actions(shop, actions),
            Phase::Unsupported(_) => actions.push(RunAction::Proceed),
            Phase::GameOver | Phase::Victory => {}
        }
    }

    /// The actions of the reward screen: each reward's own, then the way on.
    fn push_reward_actions(&self, rewards: &[Reward], actions: &mut Vec<RunAction>) {
        for (i, reward) in rewards.iter().enumerate() {
            match reward {
                Reward::Card(cards) => {
                    let alternatives = self.alternatives_of(cards);
                    if alternatives.len() <= MAX_CARD_ALTERNATIVES {
                        actions.extend(
                            (0..cards.cards.len())
                                .map(|card| RunAction::PickCard { reward: i, card }),
                        );
                        actions.extend(
                            alternatives
                                .iter()
                                .enumerate()
                                .filter(|(_, a)| **a != CardAlternative::Skip)
                                .map(|(alternative, _)| RunAction::CardAlternative {
                                    reward: i,
                                    alternative,
                                }),
                        );
                    }
                }
                Reward::Potion(_)
                    if !self.state.potions.contains(&None)
                        || !hooks::should_procure_potion(&self.state) => {}
                _ => actions.push(RunAction::ClaimReward(i)),
            }
            if self.draft_left == 0 {
                actions.push(RunAction::SkipReward(i));
            }
        }
        if self.draft_left == 0 {
            actions.push(RunAction::Proceed);
        }
        self.push_potion_actions(actions);
    }

    /// The actions of a card selection: the cards still to pick, confirm, cancel, and the
    /// picks that can go back.
    fn push_select_actions(select: &CardSelect, actions: &mut Vec<RunAction>) {
        actions.extend(
            (0..select.options.len())
                .filter(|i| !select.picked.contains(i))
                .filter(|_| select.picked.len() < select.count)
                .map(RunAction::SelectCard),
        );
        if select.can_confirm() {
            actions.push(RunAction::ConfirmSelect);
        }
        if select.purpose.is_cancelable() {
            actions.push(RunAction::CancelSelect);
        }
        actions.extend(select.picked.iter().copied().map(RunAction::DeselectCard));
    }

    /// Applies a player action.
    ///
    /// # Errors
    /// If the action does not answer the pending decision or its content is not ported; the run
    /// is then unchanged.
    pub fn apply(&mut self, action: RunAction) -> Result<(), RunError> {
        if self.applies_in_place(&action) {
            self.apply_in_place(action)
        } else {
            // Everything else may fail after changing the run (`rollback`).
            self.apply_atomic(action)
        }
    }

    /// Whether `action` changes nothing when it fails: a potion discard, or an action in the
    /// current combat (`Combat::apply` changes nothing when it fails).
    const fn applies_in_place(&self, action: &RunAction) -> bool {
        matches!(action, RunAction::DiscardPotion(_))
            || matches!(
                (&self.phase, action),
                (Phase::Combat(_), RunAction::Combat(_))
            )
    }

    /// [`Run::apply`] in place, without the journal.
    fn apply_unjournaled(&mut self, action: &RunAction) -> Result<(), RunError> {
        self.ended_combat = None;
        if let RunAction::DiscardPotion(slot) = *action {
            return self.discard_potion(slot);
        }
        if let (Phase::Combat(combat), RunAction::Combat(action)) = (&mut self.phase, action) {
            let status = combat.apply(action.clone())?;
            return self.settle_combat(status);
        }
        self.apply_out_of_combat(action.clone())
    }

    /// Takes the `i`th reward of the reward screen (a card reward is picked, not claimed).
    fn claim_reward(&mut self, i: usize) -> Result<(), RunError> {
        let Phase::Rewards(rewards) = &self.phase else {
            return Err(RunError::IllegalAction);
        };
        let reward = rewards.get(i).cloned().ok_or(RunError::IllegalAction)?;
        match reward {
            // `GoldReward.OnSelect`: `PlayerCmd.GainGold`.
            Reward::Gold(amount) => self.state.gain_gold(Decimal::from(amount)),
            Reward::Potion(id) => {
                if !self.state.procure_potion(id) {
                    return Err(RunError::IllegalAction);
                }
            }
            // `RelicReward.OnSelect`: `RelicCmd.Obtain`; the reward is taken once its
            // `AfterObtained` (and any prompt it opens) is done.
            Reward::Relic(id) | Reward::WaxRelic(id) => {
                let wax = matches!(reward, Reward::WaxRelic(_));
                let outer = self.after_prompt.take();
                let prompt = self.obtain_relic_as(id, None, wax)?;
                self.modify_card_rewards_for_new_relic();
                self.remove_reward(i);
                self.open_over_screen(prompt, outer);
                return Ok(());
            }
            // `SpecialCardReward.OnSelect`: `CardPileCmd.Add(card, Deck)`.
            Reward::SpecialCard(card) => self.state.add_card(card),
            Reward::Card(_) => return Err(RunError::IllegalAction),
        }
        self.remove_reward(i);
        Ok(())
    }

    fn apply_out_of_combat(&mut self, action: RunAction) -> Result<(), RunError> {
        if let RunAction::UsePotion(slot) = action {
            if !self.potion_actions().contains(&action) {
                return Err(RunError::IllegalAction);
            }
            return self.use_potion(slot);
        }
        match (&mut self.phase, action) {
            (Phase::Ancient(_), RunAction::AncientOption(index)) => {
                self.choose_ancient_option(index)
            }
            (Phase::Event(_), RunAction::EventOption(index)) => self.choose_event_option(index),
            (Phase::Map, RunAction::ChooseNode(coord)) => self.enter_node(coord),
            (Phase::Rewards(_), RunAction::ClaimReward(i)) => self.claim_reward(i),
            (Phase::Rewards(_), RunAction::PickCard { reward, card }) => {
                self.pick_reward_card(reward, card)
            }
            (
                Phase::Rewards(_),
                RunAction::CardAlternative {
                    reward,
                    alternative,
                },
            ) => self.choose_card_alternative(reward, alternative),
            (Phase::Rewards(rewards), RunAction::SkipReward(i)) => {
                if i >= rewards.len() {
                    return Err(RunError::IllegalAction);
                }
                self.remove_reward(i);
                Ok(())
            }
            (Phase::CardSelect(_), RunAction::SelectCard(i)) => self.select_card(i),
            (Phase::CardSelect(select), RunAction::DeselectCard(i)) => {
                let at = select
                    .picked
                    .iter()
                    .position(|&p| p == i)
                    .ok_or(RunError::IllegalAction)?;
                select.picked.remove(at);
                Ok(())
            }
            (Phase::CardSelect(select), RunAction::ConfirmSelect) if select.can_confirm() => {
                let select = select.clone();
                self.finish_select(&select)
            }
            // `CardPileCmd.Add` per card of the bundle.
            (Phase::Bundles(bundles), RunAction::SelectBundle(i)) => {
                let bundle = bundles.get(i).cloned().ok_or(RunError::IllegalAction)?;
                for card in bundle {
                    self.state.add_card(card);
                }
                self.finish_prompt()
            }
            (Phase::CardSelect(select), RunAction::CancelSelect) => {
                let purpose = select.purpose;
                self.cancel_select(purpose)
            }
            (Phase::Shop(_), action) => {
                if !self.legal_actions().contains(&action) {
                    return Err(RunError::IllegalAction);
                }
                self.shop_action(&action)
            }
            (Phase::RestSite(rest), RunAction::RestOption(i)) => {
                let rest = rest.clone();
                self.choose_rest_option(rest, i)
            }
            (Phase::RestSite(rest), RunAction::Proceed) if rest.used => {
                self.leave_room();
                Ok(())
            }
            (Phase::Treasure(relic), RunAction::TakeRelic) => {
                let id = relic.ok_or(RunError::IllegalAction)?;
                let outer = self.after_prompt.take();
                let prompt = self.obtain_relic(id)?;
                self.phase = Phase::Treasure(None);
                self.open_over_screen(prompt, outer);
                Ok(())
            }
            (Phase::Rewards(_), RunAction::Proceed) => self.leave_rewards(),
            (Phase::Treasure(_) | Phase::Unsupported(_), RunAction::Proceed) => {
                self.leave_room();
                Ok(())
            }
            _ => Err(RunError::IllegalAction),
        }
    }

    /// `CardReward.OnRelicObtained`: the relic just obtained modifies the card rewards still on
    /// the screen with each reward's own `Options` (a reward of given cards does not listen).
    fn modify_card_rewards_for_new_relic(&mut self) {
        let me = self.state.relics.len() - 1;
        let Phase::Rewards(rewards) = &mut self.phase else {
            return;
        };
        for reward in rewards {
            if let Reward::Card(CardReward {
                cards,
                options: Some(options),
                ..
            }) = reward
            {
                hooks::modify_card_reward_for_obtained_relic(&mut self.state, me, cards, *options);
            }
        }
    }

    /// Takes card `card` of the card reward at index `reward`: `CardPileCmd.Add(card, Deck)`.
    fn pick_reward_card(&mut self, reward: usize, card: usize) -> Result<(), RunError> {
        let Phase::Rewards(rewards) = &self.phase else {
            return Err(RunError::IllegalAction);
        };
        let Some(Reward::Card(cards)) = rewards.get(reward) else {
            return Err(RunError::IllegalAction);
        };
        if self.alternatives_of(cards).len() > MAX_CARD_ALTERNATIVES {
            return Err(RunError::IllegalAction);
        }
        let picked = *cards.cards.get(card).ok_or(RunError::IllegalAction)?;
        self.state.add_card(picked);
        self.remove_reward(reward);
        if self.draft_left > 0 {
            self.draft_left -= 1;
            if self.draft_left > 0 {
                self.offer_draft_reward();
            } else {
                modifiers::remove_pandoras_box(&mut self.state);
                self.next_neow_page();
            }
        }
        Ok(())
    }

    /// After a relic was obtained from the current rewards screen or chest (already updated):
    /// the prompt its `AfterObtained` opened goes on top and the screen waits, with `outer` (the
    /// `after_prompt` it waited on), until the prompt ends ([`Run::finish_prompt`]). Without a
    /// prompt the screen stays and `outer` is put back.
    fn open_over_screen(&mut self, prompt: Option<Phase>, outer: Option<AfterPrompt>) {
        match prompt {
            Some(prompt) => {
                let screen = std::mem::replace(&mut self.phase, prompt);
                self.suspended_screens.push((screen, outer));
            }
            None => self.after_prompt = outer,
        }
    }

    /// `CardRewardAlternative.Generate` for the card reward at index `reward` of the rewards
    /// screen: `Skip`, then `REROLL` while `CanReroll`, then the relics' alternatives. Empty if
    /// there is no such card reward. With more than two the game throws when the reward is
    /// opened, so it offers neither its cards nor its alternatives.
    #[must_use]
    pub fn card_reward_alternatives(&self, reward: usize) -> Vec<CardAlternative> {
        match &self.phase {
            Phase::Rewards(rewards) => match rewards.get(reward) {
                Some(Reward::Card(cards)) => self.alternatives_of(cards).into_vec(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    fn alternatives_of(&self, reward: &CardReward) -> CardAlternatives {
        // `CanSkip` is only false for the `Draft` modifier's rewards.
        let mut alternatives = CardAlternatives::new();
        if self.draft_left == 0 {
            alternatives.push(CardAlternative::Skip);
        }
        if reward.can_reroll {
            alternatives.push(CardAlternative::Reroll);
        }
        hooks::modify_card_reward_alternatives(&self.state, &mut alternatives);
        alternatives
    }

    /// Chooses an alternative of a card reward (`CardReward.OnSelect`): `REROLL`
    /// (`CardReward.Reroll`: `CanReroll` cleared, the cards replaced by `CreateForReward` with
    /// `RerollOptions` and `OptionCount`; the screen stays), or a relic's (its callback, then the
    /// reward is complete). A relic obtained meanwhile modifies the card rewards on the screen.
    fn choose_card_alternative(
        &mut self,
        index: usize,
        alternative: usize,
    ) -> Result<(), RunError> {
        let Phase::Rewards(rewards) = &self.phase else {
            return Err(RunError::IllegalAction);
        };
        let Some(Reward::Card(reward)) = rewards.get(index) else {
            return Err(RunError::IllegalAction);
        };
        let alternatives = self.alternatives_of(reward);
        if alternatives.len() > MAX_CARD_ALTERNATIVES {
            return Err(RunError::IllegalAction);
        }
        match alternatives.get(alternative) {
            Some(CardAlternative::Reroll) => {
                let mut reward = reward.clone();
                // `Kaleidoscope`'s reroll options have no card pool.
                let options = reward
                    .reroll_options
                    .ok_or(RunError::UnsupportedRelic("DRIFTWOOD"))?;
                reward.can_reroll = false;
                let rng = if reward.event_rng {
                    let event = self
                        .suspended_event
                        .as_mut()
                        .expect("an event's reward waits behind it");
                    Some(&mut event.rng)
                } else {
                    None
                };
                reward.cards =
                    rewards::create_card_reward_on(&mut self.state, reward.count, options, rng);
                if reward.upgrade_generated {
                    reward.cards.iter_mut().for_each(RunCard::upgrade);
                }
                if let Phase::Rewards(rewards) = &mut self.phase {
                    rewards[index] = Reward::Card(reward);
                }
                Ok(())
            }
            Some(&CardAlternative::Relic { relic, option }) => {
                let relics = self.state.relics.len();
                let kind = self.state.relics[relic].kind;
                let outer = self.after_prompt.take();
                let mut ctx = RunCtx::new(self);
                kind.model()
                    .on_card_reward_alternative(&mut ctx, relic, option)?;
                let (prompt, after) = (ctx.prompt.map(prompt_phase), ctx.after);
                match after {
                    Some(after) if prompt.is_some() => self.after_prompt = Some(after),
                    Some(after) => self.run_after_prompt(after),
                    None => {}
                }
                if self.state.relics.len() > relics {
                    self.modify_card_rewards_for_new_relic();
                }
                self.remove_reward(index);
                self.open_over_screen(prompt, outer);
                Ok(())
            }
            Some(CardAlternative::Skip) | None => Err(RunError::IllegalAction),
        }
    }

    /// Cancels (or skips) the pending card selection.
    fn cancel_select(&mut self, purpose: SelectPurpose) -> Result<(), RunError> {
        match purpose {
            SelectPurpose::Smith | SelectPurpose::Cook => {
                let mut rest = self.suspended_rest.take().expect("Smith has its rest site");
                rest.pending = None;
                self.phase = Phase::RestSite(rest);
            }
            SelectPurpose::ShopRemoval { .. } => {
                let shop = self.suspended_shop.take().expect("a removal has its shop");
                self.phase = Phase::Shop(shop);
            }
            // Skipped: nothing is picked, the extra card still joins.
            SelectPurpose::ChooseACard { extra } => {
                if let Some(id) = extra {
                    self.state.add_card(RunCard::new(id));
                }
                self.finish_prompt()?;
            }
            _ => return Err(RunError::IllegalAction),
        }
        Ok(())
    }

    /// Chooses option `i` of the rest site.
    fn choose_rest_option(&mut self, mut rest: RestSite, i: usize) -> Result<(), RunError> {
        let option = *rest.options.get(i).ok_or(RunError::IllegalAction)?;
        if !option.enabled {
            return Err(RunError::IllegalAction);
        }
        match option.kind {
            RestOptionKind::Heal => {
                // `ExecuteRestSiteHeal`: the heal, then `OfferCustom` of the rewards the
                // listeners add (none: nothing is shown).
                let rewards = self.state.rest_site_heal(false);
                rest.options.remove(i);
                let mut ctx = RunCtx::new(self);
                if !rewards.is_empty() {
                    ctx.offer_pending_rewards(rewards);
                }
                let prompt = ctx.prompt;
                self.rest_option_done(rest, prompt, None);
            }
            RestOptionKind::Smith => {
                // The option is removed once the selection succeeds.
                rest.pending = Some(RestOptionKind::Smith);
                self.suspended_rest = Some(rest);
                self.phase = Phase::CardSelect(CardSelect {
                    purpose: SelectPurpose::Smith,
                    options: self.state.upgradable_cards(),
                    offered: Vec::new(),
                    count: SMITH_COUNT,
                    picked: Vec::new(),
                });
            }
            // `LiftRestSiteOption.OnSelect`: `Girya.TimesLifted++`.
            RestOptionKind::Lift => {
                let girya = self
                    .state
                    .relics
                    .iter()
                    .position(|r| r.kind == RelicKind::Girya)
                    .ok_or(RunError::IllegalAction)?;
                girya::lift(&mut self.state, girya);
                rest.options.remove(i);
                self.after_rest_option(rest);
            }
            // `DigRestSiteOption.OnSelect`: `RelicCmd.Obtain(PullNextRelicFromFront)`.
            RestOptionKind::Dig => {
                let id = rewards::pull_relic_from_front(&mut self.state);
                let prompt = self.obtain_relic(id)?;
                rest.options.remove(i);
                self.rest_option_done(rest, None, prompt);
            }
            // `HatchRestSiteOption.OnSelect`: `RelicCmd.Obtain<Byrdpip>`.
            RestOptionKind::Hatch => {
                let prompt = self.obtain_relic("BYRDPIP")?;
                rest.options.remove(i);
                self.rest_option_done(rest, None, prompt);
            }
            // `KindleRestSiteOption.OnSelect`: `Owner.GetRelic<PumpkinCandle>()?.Rekindle()`.
            RestOptionKind::Kindle => {
                if let Some(candle) = self
                    .state
                    .relics
                    .iter()
                    .position(|r| r.kind == RelicKind::PumpkinCandle)
                {
                    pumpkin_candle::rekindle(&mut self.state, candle);
                }
                rest.options.remove(i);
                self.after_rest_option(rest);
            }
            // `CloneRestSiteOption.OnSelect`: `CardPileCmd.Add(RunState.CloneCard(card), Deck)`
            // for each deck card enchanted with `Clone`, as the deck was before the first copy.
            RestOptionKind::Clone => {
                let cards: Vec<RunCard> = self
                    .state
                    .deck
                    .iter()
                    .filter(|c| {
                        c.enchantment
                            .is_some_and(|e| e.kind == EnchantmentKind::Clone)
                    })
                    .copied()
                    .collect();
                for card in cards {
                    self.state.add_card(card);
                }
                rest.options.remove(i);
                self.after_rest_option(rest);
            }
            // `CookRestSiteOption.OnSelect`: `FromDeckForRemoval` of 2, cancelable and with manual
            // confirmation (it always prompts); the option is removed once the selection succeeds.
            RestOptionKind::Cook => {
                rest.pending = Some(RestOptionKind::Cook);
                self.suspended_rest = Some(rest);
                self.phase = Phase::CardSelect(CardSelect {
                    purpose: SelectPurpose::Cook,
                    options: self.state.removable_cards(),
                    offered: Vec::new(),
                    count: COOK_COUNT,
                    picked: Vec::new(),
                });
            }
        }
        Ok(())
    }

    /// A rest-site option ran: if it opened a prompt (a rewards screen, or the screen of a relic
    /// it obtained) the rest site waits for it, else [`Run::after_rest_option`].
    fn rest_option_done(
        &mut self,
        rest: RestSite,
        rewards: Option<Prompt>,
        relic_prompt: Option<Phase>,
    ) {
        let phase = relic_prompt.or_else(|| {
            rewards.map(|prompt| match prompt {
                Prompt::Rewards(rewards) => Phase::Rewards(rewards),
                _ => unreachable!("a heal only offers rewards"),
            })
        });
        match phase {
            Some(phase) => {
                self.suspended_rest = Some(rest);
                self.phase = phase;
            }
            None => self.after_rest_option(rest),
        }
    }

    /// Leaving a rewards screen (`NRewardsScreen.OnProceedButtonPressed`, `NRewardsScreen.cs:566-586`):
    /// after a boss's, the first of the last act's two bosses (`DoubleBoss`: the map has a second
    /// boss point and the player is on the boss point) returns to the map
    /// (`ProceedFromTerminalRewardsScreen`), whose only next point is the second boss; any other
    /// boss moves the run on (`ActChangeSynchronizer.MoveToNextAct`, [`Run::enter_next_act`]).
    /// Else [`Run::finish_prompt`].
    fn leave_rewards(&mut self) -> Result<(), RunError> {
        let boss = matches!(
            self.state.room,
            Room::Combat {
                room_type: RoomType::Boss,
                ..
            }
        );
        let s = &self.state;
        let first_of_two = s.map.second_boss().is_some() && s.coord() == Some(s.map.boss().coord);
        if boss
            && self.suspended_event.is_none()
            && self.suspended_screens.is_empty()
            && !first_of_two
        {
            return self.enter_next_act();
        }
        self.finish_prompt()
    }

    /// `ActChangeSynchronizer.MoveToNextAct` (`ActChangeSynchronizer.cs:80-94`): `ActFloor++`,
    /// then `RunManager.EnterNextAct` (`RunManager.cs:1285-1310`): before the last act
    /// [`Run::enter_act`] of the next act; after it `TheArchitect`'s event room
    /// ([`Run::enter_victory_room`]). No HP is restored between acts (the act's ancient heals:
    /// [`Run::begin_ancient`]).
    fn enter_next_act(&mut self) -> Result<(), RunError> {
        let s = &mut self.state;
        s.act_floor += 1;
        if s.act_index + 1 >= s.acts.len() {
            self.enter_victory_room();
            return Ok(());
        }
        let next = s.act_index + 1;
        self.enter_act(next)
    }

    /// `RunManager.EnterAct(index)` → `SetActInternal` (`RunManager.cs:1321-1370`): the rooms are
    /// exited, `CurrentActIndex` is set (visited coords cleared, so `ActFloor` is 0), the `?` odds
    /// reset to base, the act's map is generated ([`RunState::generate_map`]), and a `MapRoom` is
    /// entered (no room hooks, nothing counted). `AfterActEntered` has no listener outside
    /// modifiers. The player is on the new map, whose only travelable point is the start (the
    /// act's ancient).
    fn enter_act(&mut self, index: usize) -> Result<(), RunError> {
        let s = &mut self.state;
        s.act_index = index;
        s.visited.clear();
        s.act_floor = 0;
        s.unknown_odds.reset_to_base();
        s.generate_map().map_err(RunError::Map)?;
        self.leave_room();
        Ok(())
    }

    /// `RunManager.EnterRoom(new EventRoom(TheArchitect))` (`RunManager.cs:1302`): the rooms are
    /// exited and the event room entered (`EnterRoomInternal`, `RunManager.cs:1169-1218`; not a map
    /// point, so no history entry). `EventRoom.EnterInternal` begins the event (its dialogue is
    /// picked on the event's own `Rng` from the profile's win counts, `TheArchitect.cs:311-317`:
    /// no run stream draws). Its `EventLayoutType.Combat` creates the `Architect` for the layout
    /// (`GenerateInternalCombatStateIfNecessary`, `EventRoom.cs:72`), one `Niche` draw for its
    /// unique max HP (`CombatState.CreateCreature`, as for `TheLanternKey`'s knight). Then `AfterRoomEntered` (`EventRoom.cs:83-86`) and `MarkRoomVisited(Event)`.
    /// `BeforeRoomEntered` has no listener for a room off the map. The run is won
    /// ([`RunDecision::Victory`]): every option of the room leads to `WinRun`.
    fn enter_victory_room(&mut self) {
        self.state.room = Room::Event { id: THE_ARCHITECT };
        self.state.rng[RunRngType::Niche].next_int(1);
        hooks::after_room_entered(&mut self.state, RoomType::Event);
        self.state.act_mut().events_visited += 1;
        self.phase = Phase::Victory;
    }

    /// Back to the map screen (`MapRoom`).
    fn leave_room(&mut self) {
        self.state.room = Room::Map;
        self.phase = Phase::Map;
    }

    /// The end of a rewards screen or card selection: back to the rewards screen or chest a
    /// claimed relic opened it over, else the shop (and its queued free purchases), rest site
    /// or event that opened it, if any, else to the map.
    ///
    /// # Errors
    /// As [`Run::purchase`] for a queued free purchase.
    fn finish_prompt(&mut self) -> Result<(), RunError> {
        if let Some(after) = self.after_prompt.take() {
            self.run_after_prompt(after);
        }
        if let Some((screen, outer)) = self.suspended_screens.pop() {
            self.phase = screen;
            self.after_prompt = outer;
            return Ok(());
        }
        if let Some(mut shop) = self.suspended_shop.take() {
            if let Some((i, cost)) = self.relic_purchase_after_prompt.take() {
                self.finish_relic_purchase(&mut shop, i, cost);
            }
            self.phase = Phase::Shop(shop);
            return self.continue_free_purchases();
        }
        if let Some(mut rest) = self.suspended_rest.take() {
            if let Some(kind) = rest.pending.take() {
                rest.options.retain(|o| o.kind != kind);
            }
            self.after_rest_option(rest);
            return Ok(());
        }
        match self.suspended_event.take() {
            Some(room) => self.resume_event(room),
            None if self.sealed_pending => {
                self.sealed_pending = false;
                modifiers::remove_pandoras_box(&mut self.state);
                self.next_neow_page();
            }
            None => self.leave_room(),
        }
        Ok(())
    }

    fn remove_reward(&mut self, index: usize) {
        if let Phase::Rewards(rewards) = &mut self.phase {
            rewards.remove(index);
        }
    }

    /// The points the player can travel to (`MapTravel.GetTravelablePointsFrom`): the current
    /// point's children, or with free travel (`ShouldAllowFreeTravel`) every point of the next
    /// row; from the last row only the boss (`NMapScreen`).
    fn next_nodes(&self) -> NextNodes {
        let s = &self.state;
        // A new act, no point visited yet: only its start (the ancient).
        let Some(point) = s.coord().and_then(|c| s.map.point(c)) else {
            return smallvec::smallvec![s.map.start().coord];
        };
        let row = point.coord.row + 1;
        let mut next: NextNodes =
            if hooks::should_allow_free_travel(s) && row < s.map.boss().coord.row {
                s.map
                    .points()
                    .iter()
                    .filter(|p| p.coord.row == row)
                    .map(|p| p.coord)
                    .collect()
            } else {
                point.children.iter().collect()
            };
        next.sort();
        next
    }

    /// `EnterMapCoord(start)` for Act 1's start point when the run begins: the point is visited
    /// and the history entry added, with no room hooks, then the ancient begins
    /// ([`Run::begin_ancient`]).
    fn enter_neow(&mut self, start: MapCoord) {
        let s = &mut self.state;
        s.visited.push(start);
        s.act_floor = start.row + 1;
        let ancient = s.act().ancient;
        s.history.push(MapPointHistoryEntry {
            point_type: MapPointType::Ancient,
            room_type: Some(RoomType::Event),
            model: Some(ancient),
        });
        self.phase = self.begin_ancient();
        if !self.state.modifiers.is_empty() {
            modifiers::after_base_room_entered(&mut self.state);
            // Neow with modifiers offers only theirs; with none to offer the event is over.
            if self.neow_modifiers.is_empty() {
                self.leave_room();
            }
        }
    }

    /// An `Ancient` point entered from the map (the start of acts 2 and 3): `CreateRoom` pulls the
    /// act's ancient (`PullAncient`, `RunManager.cs:958`; nothing is added to the visited events),
    /// the history entry is added, the ancient begins, then `AfterRoomEntered` if it is ported.
    fn enter_ancient(&mut self) {
        let id = self.state.act().ancient;
        self.push_history(MapPointType::Ancient, RoomType::Event, Some(id));
        self.phase = self.begin_ancient();
        if matches!(self.phase, Phase::Ancient(_)) {
            hooks::after_room_entered(&mut self.state, RoomType::Event);
        }
    }

    /// `EventRoom.EnterInternal` for the act's ancient, then `MarkRoomVisited(Event)`.
    /// `BeginEvent` runs `AncientEventModel.BeforeEventStarted` (`AncientEventModel.cs:170-191`):
    /// Neow first sets HP to 0; then the missing HP is healed (×0.8 from `WearyTraveler` on;
    /// `CreatureCmd.Heal` has no hooks). Then `GenerateInitialOptionsWrapper` rolls the options on
    /// the event `Rng` (`ShouldAllowAncient` has no override, so they are always generated). An
    /// unported ancient is a stub.
    fn begin_ancient(&mut self) -> Phase {
        let s = &mut self.state;
        let id = s.act().ancient;
        s.room = Room::Ancient { id };
        let phase = match AncientKind::lookup(id) {
            Some(kind) => {
                if kind == AncientKind::Neow {
                    s.hp = 0;
                }
                let mut amount = Decimal::from(s.max_hp - s.hp);
                if AscensionLevel::WearyTraveler.is_active(s.ascension) {
                    amount *= Decimal::new(8, 1);
                }
                s.heal(amount);
                let mut rng = events::event_rng(s.rng.seed(), kind.id());
                let options = if kind == AncientKind::Neow && !s.modifiers.is_empty() {
                    // `Neow.GenerateInitialOptions` with modifiers: the first modifier option.
                    modifiers::neow_options(s)
                        .into_iter()
                        .take(1)
                        .map(AncientOption::Modifier)
                        .collect()
                } else {
                    kind.model().generate_options(s, &mut rng)
                };
                Phase::Ancient(AncientRoom { kind, options })
            }
            None => Phase::Unsupported(MapPointType::Ancient),
        };
        s.act_mut().events_visited += 1;
        if matches!(&phase, Phase::Ancient(r) if r.kind == AncientKind::Neow)
            && !s.modifiers.is_empty()
        {
            self.neow_modifiers = modifiers::neow_options(&self.state);
        }
        phase
    }

    /// Picks an ancient's option (`AncientEventModel.RelicOption`): `RelicCmd.Obtain` (the relic
    /// joins, `AfterObtained` runs), then `Done` and the player is on the map, unless the relic
    /// prompts first (a card selection or a rewards screen).
    fn choose_ancient_option(&mut self, index: usize) -> Result<(), RunError> {
        let Phase::Ancient(room) = &self.phase else {
            return Err(RunError::IllegalAction);
        };
        let option = *room.options.get(index).ok_or(RunError::IllegalAction)?;
        if let AncientOption::Modifier(modifier) = option {
            self.choose_neow_modifier(modifier);
            return Ok(());
        }
        let AncientOption::Relic { id, data } = option else {
            return Err(RunError::IllegalAction);
        };
        if !option.is_supported() {
            return Err(RunError::UnsupportedAncientOption(id));
        }
        // The event stays open while its relic prompts.
        match self.obtain_preset_relic(id, data)? {
            Some(next) => self.phase = next,
            None => self.leave_room(),
        }
        Ok(())
    }

    /// `Neow.OnModifierOptionSelected`: the modifier's effect, then the next modifier's page, or
    /// the end of the event.
    fn choose_neow_modifier(&mut self, modifier: Modifier) {
        self.neow_modifiers.remove(0);
        match modifiers::choose_neow_option(&mut self.state, modifier) {
            NeowOutcome::Done => self.next_neow_page(),
            NeowOutcome::Draft => {
                self.draft_left = modifiers::draft::PICKS;
                self.offer_draft_reward();
            }
            NeowOutcome::Sealed(cards) => {
                self.sealed_pending = true;
                self.phase = Phase::CardSelect(CardSelect {
                    purpose: SelectPurpose::AddToDeck,
                    options: (0..cards.len()).collect(),
                    offered: cards,
                    count: modifiers::sealed_deck::KEPT,
                    picked: Vec::new(),
                });
            }
        }
    }

    /// The next modifier's Neow page, or the map once there is none.
    fn next_neow_page(&mut self) {
        match self.neow_modifiers.first() {
            Some(&next) => {
                self.phase = Phase::Ancient(AncientRoom {
                    kind: AncientKind::Neow,
                    options: vec![AncientOption::Modifier(next)],
                });
            }
            None => self.leave_room(),
        }
    }

    /// One card reward of the `Draft`, alone on the rewards screen.
    fn offer_draft_reward(&mut self) {
        let reward = modifiers::draft_reward(&mut self.state);
        self.phase = Phase::Rewards(vec![reward]);
    }

    /// `RelicCmd.Obtain` (`Commands/RelicCmd.cs:35-55`): the relic joins, leaves both bags, and
    /// its `AfterObtained` runs. Returns the phase the relic prompts with, if any.
    fn obtain_relic(&mut self, id: &'static str) -> Result<Option<Phase>, RunError> {
        self.obtain_preset_relic(id, None)
    }

    /// [`Run::obtain_relic`] with the relic's fields preset ([`RunCtx::obtain_preset_relic`]).
    fn obtain_preset_relic(
        &mut self,
        id: &'static str,
        data: Option<RelicData>,
    ) -> Result<Option<Phase>, RunError> {
        self.obtain_relic_as(id, data, false)
    }

    /// [`Run::obtain_preset_relic`], made wax first if `wax`.
    fn obtain_relic_as(
        &mut self,
        id: &'static str,
        data: Option<RelicData>,
        wax: bool,
    ) -> Result<Option<Phase>, RunError> {
        let mut ctx = RunCtx::new(self);
        ctx.obtain_relic_as(id, data, wax)?;
        let (prompt, after) = (ctx.prompt, ctx.after);
        let phase = prompt.map(prompt_phase);
        match after {
            Some(after) if phase.is_some() => self.after_prompt = Some(after),
            Some(after) => self.run_after_prompt(after),
            None => {}
        }
        Ok(phase)
    }

    /// What a hook asked to run once its prompt ended.
    fn run_after_prompt(&mut self, after: AfterPrompt) {
        match after {
            AfterPrompt::Curses(count) => neows_bones::add_curses(&mut self.state, count),
            AfterPrompt::AddCard(id) => self.state.add_card(RunCard::new(id)),
        }
    }

    fn select_card(&mut self, index: usize) -> Result<(), RunError> {
        let Phase::CardSelect(select) = &mut self.phase else {
            return Err(RunError::IllegalAction);
        };
        if index >= select.options.len()
            || select.picked.contains(&index)
            || select.picked.len() >= select.count
        {
            return Err(RunError::IllegalAction);
        }
        select.picked.push(index);
        if select.picked.len() < select.count || select.manual_confirm() {
            return Ok(());
        }
        let select = select.clone();
        self.finish_select(&select)
    }

    /// A card selection is confirmed (or has all its picks): apply its purpose.
    fn finish_select(&mut self, select: &CardSelect) -> Result<(), RunError> {
        let cards: Vec<usize> = select.picked.iter().map(|&i| select.options[i]).collect();
        match select.purpose {
            SelectPurpose::AddToDeck | SelectPurpose::AddToDeckUpTo => {
                for i in cards {
                    self.state.add_card(select.offered[i]);
                }
            }
            SelectPurpose::ChooseACard { extra } => {
                for i in cards {
                    self.state.add_card(select.offered[i]);
                }
                if let Some(id) = extra {
                    self.state.add_card(RunCard::new(id));
                }
            }
            // The event takes over: it resumes, or opens its next prompt.
            SelectPurpose::Event { action, .. } => {
                return self.resume_event_selection(action, &cards);
            }
            purpose => self.resolve_deck_purpose(purpose, &cards)?,
        }
        self.finish_prompt()
    }

    /// Applies a finished deck selection: `cards` are the picked deck indices in pick order.
    ///
    /// # Errors
    /// [`RunError::UnsupportedCard`] for a transform of a card whose pool is not modeled.
    ///
    /// # Panics
    /// For [`SelectPurpose::AddToDeck`], [`SelectPurpose::ChooseACard`] and
    /// [`SelectPurpose::Event`], which are not deck commands.
    fn resolve_deck_purpose(
        &mut self,
        purpose: SelectPurpose,
        cards: &[usize],
    ) -> Result<(), RunError> {
        let s = &mut self.state;
        match purpose {
            // `CardCmd.Upgrade` per card; Smith then runs `AfterRestSiteSmith`.
            SelectPurpose::Upgrade => {
                for &i in cards {
                    s.deck[i].upgrade();
                }
            }
            SelectPurpose::Smith => {
                for &i in cards {
                    s.deck[i].upgrade();
                }
                hooks::after_rest_site_smith(s);
            }
            // `CookRestSiteOption.OnSelect`: `RemoveFromDeck` per card, then `GainMaxHp(5)`.
            SelectPurpose::Cook => {
                s.remove_cards(cards);
                s.gain_max_hp(COOK_MAX_HP);
            }
            // `CardPileCmd.RemoveFromDeck` per card, then `CreatureCmd.Damage` (unblocked
            // outside combat).
            SelectPurpose::Remove { damage } => {
                s.remove_cards(cards);
                s.damage(damage);
            }
            // `DoMerchantCardRemoval`: `LoseGold`, `RemoveFromDeck`, then the count; the
            // entry is `Used` (`NMerchantCardRemoval.OnCardRemovalUsed`).
            SelectPurpose::ShopRemoval { cost, .. } => {
                s.lose_gold(cost);
                s.remove_cards(cards);
                s.card_shop_removals_used += 1;
                if let Some(shop) = &mut self.suspended_shop {
                    shop.removal_used = true;
                }
                // `MerchantCardRemovalEntry.OnTryPurchaseWrapper`: once the removal is done.
                hooks::after_item_purchased(s, cost);
            }
            SelectPurpose::Enchant { kind, amount }
            | SelectPurpose::EnchantUpTo { kind, amount } => {
                for &i in cards {
                    s.enchant(i, kind, amount)
                        .expect("only enchantable deck cards are offered");
                }
            }
            // `CardCmd.TransformToRandom(card, Rng.Niche)` per card (one for `NewLeaf`).
            SelectPurpose::TransformRandom => {
                let mut rng = s.rng[RunRngType::Niche].clone();
                s.transform_many_to_random(cards, &mut rng)?;
                s.rng[RunRngType::Niche] = rng;
            }
            SelectPurpose::TransformRandomUpgraded => {
                let mut rng = s.rng[RunRngType::Niche].clone();
                let result = s.transform_each_to_random_upgraded(cards, &mut rng);
                s.rng[RunRngType::Niche] = rng;
                result?;
            }
            // `CardPileCmd.Add(RunState.CloneCard(card), Deck)`.
            SelectPurpose::Clone => {
                for &i in cards {
                    s.add_card(s.deck[i]);
                }
            }
            SelectPurpose::Relic { relic } | SelectPurpose::RelicUpTo { relic } => {
                s.relics[relic]
                    .kind
                    .model()
                    .on_cards_selected(s, relic, cards);
            }
            SelectPurpose::AddToDeck
            | SelectPurpose::AddToDeckUpTo
            | SelectPurpose::ChooseACard { .. }
            | SelectPurpose::Event { .. } => {
                unreachable!("not a deck command")
            }
        }
        Ok(())
    }

    /// `EnterMapCoord` → `EnterMapPointInternal` (`RunManager.cs:833-938`): visit the coord, set
    /// `ActFloor`, roll the room type (a `?` rolls on `UnknownMapPoint`), create the room
    /// (pulling the next encounter), append the history entry, enter the room, then
    /// `MarkRoomVisited` by the room type.
    fn enter_node(&mut self, coord: MapCoord) -> Result<(), RunError> {
        if !self.next_nodes().contains(&coord) {
            return Err(RunError::IllegalAction);
        }
        self.enter_point(coord)?;
        self.finished_event = None;
        // A fight counts it in `enter_combat`, before the relics move into the combat.
        if !matches!(self.phase, Phase::Combat(_)) {
            hooks::after_map_room_entered(&mut self.state);
        }
        Ok(())
    }

    /// [`Run::enter_node`] up to the room being entered.
    fn enter_point(&mut self, coord: MapCoord) -> Result<(), RunError> {
        let point = self.state.map.point(coord).ok_or(RunError::IllegalAction)?;
        let point_type = point.kind;
        // `BuildRoomTypeBlacklist`: no shop right after a shop or before only shops.
        let after_shop = self
            .state
            .history
            .last()
            .is_some_and(|h| h.room_type == Some(RoomType::Shop));
        let before_shops = !point.children.is_empty()
            && point.children.iter().all(|c| {
                self.state
                    .map
                    .point(c)
                    .is_some_and(|p| p.kind == MapPointType::Shop)
            });
        let s = &mut self.state;
        s.visited.push(coord);
        s.act_floor = coord.row + 1;
        let room_type = if let Some(room_type) = RoomType::for_point(point_type) {
            room_type
        } else {
            let mut types = UnknownMapPointOdds::room_types();
            if after_shop || before_shops {
                types.retain(|&t| t != RoomType::Shop);
            }
            hooks::modify_unknown_map_point_room_types(s, &mut types);
            s.unknown_odds
                .roll(&types, &mut s.rng[RunRngType::UnknownMapPoint])
        };
        match room_type {
            RoomType::Monster | RoomType::Elite | RoomType::Boss => {
                self.enter_combat(coord, point_type, room_type)
            }
            RoomType::RestSite => {
                self.push_history(point_type, room_type, None);
                self.state.room = Room::RestSite;
                // `RestSiteRoom.EnterInternal`: the options, then `AfterRoomEntered`.
                let options = self.rest_options();
                hooks::after_room_entered(&mut self.state, RoomType::RestSite);
                self.phase = Phase::RestSite(RestSite {
                    options,
                    used: false,
                    pending: None,
                });
                Ok(())
            }
            RoomType::Treasure => {
                self.push_history(point_type, room_type, None);
                self.enter_treasure();
                Ok(())
            }
            RoomType::Event if point_type == MapPointType::Ancient => {
                self.enter_ancient();
                Ok(())
            }
            RoomType::Event => self.enter_event(point_type),
            RoomType::Shop => {
                self.push_history(point_type, room_type, None);
                // `MerchantRoom.EnterInternal`: the inventory, then `AfterRoomEntered`.
                self.state.room = Room::Shop;
                let mut shop = Shop::generate(&mut self.state);
                hooks::after_room_entered(&mut self.state, RoomType::Shop);
                hooks::after_merchant_room_entered(&self.state, &mut shop);
                self.phase = Phase::Shop(Box::new(shop));
                self.continue_free_purchases()
            }
            RoomType::Map => {
                self.push_history(point_type, room_type, None);
                self.state.room = Room::Unsupported {
                    point_type,
                    room_type,
                };
                self.phase = Phase::Unsupported(point_type);
                Ok(())
            }
        }
    }

    /// An event room: `CreateRoom` pulls the next event, the history entry is added, the event
    /// begins (a stub if it is not ported), then `MarkRoomVisited(Event)` counts it.
    fn enter_event(&mut self, point_type: MapPointType) -> Result<(), RunError> {
        let id = self.pull_next_event();
        self.push_history(point_type, RoomType::Event, Some(id));
        self.state.room = Room::Event { id };
        self.phase = match EventKind::from_id(id) {
            Some(kind) => self.begin_event(kind)?,
            None => Phase::Unsupported(point_type),
        };
        self.state.act_mut().events_visited += 1;
        Ok(())
    }

    /// `AppendToMapPointHistory`, then `EnterRoom` up to the room's own `Enter`: the rooms are
    /// exited, the new one pushed and `BeforeRoomEntered` runs (`RunManager.cs:1169-1237`).
    fn push_history(
        &mut self,
        point_type: MapPointType,
        room_type: RoomType,
        model: Option<&'static str>,
    ) {
        self.state.history.push(MapPointHistoryEntry {
            point_type,
            room_type: Some(room_type),
            model,
        });
        hooks::before_room_entered(&mut self.state, point_type);
    }

    /// A combat room: `CreateRoom` pulls the next encounter of the type, the fight starts, then
    /// `MarkRoomVisited` counts it.
    fn enter_combat(
        &mut self,
        coord: MapCoord,
        point_type: MapPointType,
        room_type: RoomType,
    ) -> Result<(), RunError> {
        let info = match room_type {
            RoomType::Elite => self.state.act().next_elite_encounter(),
            RoomType::Boss => self.state.act().next_boss_encounter(),
            _ => self.state.act().next_normal_encounter(),
        };
        let kind = Encounter::from_entry(info.id).ok_or(RunError::UnsupportedEncounter(info.id))?;
        let room_type = encounter_room_type(info.room);
        self.push_history(point_type, room_type, Some(info.id));
        // `AfterRoomEntered` is a combat room's first hook: the run-state part (`Planisphere`'s
        // heal) comes before the fight is set up.
        hooks::after_map_combat_room_entered(&mut self.state, point_type);
        hooks::after_map_room_entered(&mut self.state);
        let config = self.combat_config(kind, room_type, Some(coord))?;
        self.state.room = Room::Combat {
            room_type,
            encounter: info.id,
        };
        let combat = Combat::new(config);
        let act = self.state.act_mut();
        match room_type {
            RoomType::Monster => act.normal_encounters_visited += 1,
            RoomType::Elite => act.elite_encounters_visited += 1,
            _ => act.boss_encounters_visited += 1,
        }
        self.start_combat(combat)
    }

    /// `EventCombatSynchronizer.EnterCombat` (`EnterRoomWithoutExitingCurrentRoom`): a
    /// `CombatRoom` for `encounter` pushed on the event's room, at the same map point. The room
    /// is recorded in the point's history entry only (no new floor) and, as the room stack is
    /// deeper than one, `MarkRoomVisited` does not count it.
    pub(super) fn enter_event_combat(
        &mut self,
        encounter: Encounter,
        extra_rewards: Vec<PendingReward>,
    ) -> Result<(), RunError> {
        let id = encounter.entry();
        let room_type =
            rooms::room_type_of_encounter(id).ok_or(RunError::UnsupportedEncounter(id))?;
        // A forced event (`Run::debug_enter_event` on a new act's map) has no point: the game's
        // monster seeds read `CurrentMapCoord?.col ?? 0` (`CombatState.CreateCreature`).
        let coord = self.state.coord();
        let config = self.combat_config(encounter, room_type, coord)?;
        self.state.room = Room::Combat {
            room_type,
            encounter: id,
        };
        self.combat_extra_rewards = extra_rewards;
        self.start_combat(Combat::new(config))
    }

    /// A fight just set up becomes the phase. `StartTurn` checks the win condition once the
    /// first turn's start hooks have run (`CombatManager.cs:827`), so the fight can be over
    /// before the first play: `FurCoat`'s 1-HP enemies and `FestivePopper`'s first-turn damage.
    fn start_combat(&mut self, combat: Combat) -> Result<(), RunError> {
        let status = combat.status();
        self.phase = Phase::Combat(Box::new(combat));
        self.settle_combat(status)
    }

    /// Ends the fight in the combat phase if `status` says it is over.
    ///
    /// # Errors
    /// If a won fight's victory meets content that is not ported ([`Run::win_combat`]), with
    /// the run changed: the caller restores it.
    fn settle_combat(&mut self, status: Status) -> Result<(), RunError> {
        match status {
            Status::Won => self.win_combat(),
            Status::Lost => {
                self.leave_combat();
                self.phase = Phase::GameOver;
                Ok(())
            }
            Status::PlayerTurn | Status::Decision | Status::Unsupported(_) => Ok(()),
        }
    }

    /// The combat set-up for `encounter` at `coord`, built from the run state: the run's streams,
    /// deck, relics, belt, HP, and `TotalFloor`/act/coord for the encounter and monster RNGs.
    /// The streams move into the combat, which owns them until [`Run::leave_combat`]; nothing
    /// moves if the set-up fails.
    fn combat_config(
        &mut self,
        encounter: Encounter,
        room_type: RoomType,
        coord: Option<MapCoord>,
    ) -> Result<CombatConfig, RunError> {
        let s = &self.state;
        let deck = s
            .deck
            .iter()
            .map(|c| c.to_combat().ok_or(RunError::UnsupportedCard(c.id)))
            .collect::<Result<Vec<DeckCard>, _>>()?;
        if let Some(id) =
            s.potions.iter().flatten().find(|id| {
                potions::combat_potion(id).is_none() && potions::is_passive_in_combat(id)
            })
        {
            return Err(RunError::UnsupportedPotion(id));
        }
        let to_i32 = |n: usize| i32::try_from(n).expect("map and floor numbers fit i32");
        let rng = std::mem::replace(&mut self.state.rng, RunRngSet::placeholder());
        if self.record_start_streams {
            match &mut self.combat_start_streams {
                Some(copy) => (**copy).clone_from(&rng),
                None => self.combat_start_streams = Some(Box::new(rng.clone())),
            }
        }
        let s = &self.state;
        Ok(CombatConfig {
            character: s.character,
            rng,
            ascension: s.ascension,
            encounter,
            room_type,
            deck,
            relics: relics::combat_relics(&s.relics),
            potion_slots: s.potions.len(),
            potions: s
                .potions
                .iter()
                .map(|p| {
                    p.map(|id| {
                        potions::combat_potion(id)
                            .map_or(BeltPotion::Unported(id), BeltPotion::Ported)
                    })
                })
                .collect(),
            player_hp: s.hp,
            player_max_hp: s.max_hp,
            max_energy: s.max_energy,
            gold: s.gold,
            total_floor: to_i32(s.total_floor()),
            act_index: to_i32(s.act_index),
            map_coord: coord.map(|c| (to_i32(c.col), to_i32(c.row))),
            record_events: self.record_events,
            modifiers: modifiers::CombatModifiers::of(&s.modifiers),
        })
    }

    /// The end of a fight: the run takes back the streams and keeps the player's HP and max HP,
    /// gold (`Debt` loses it in a fight), the belt, the relics' fields and the deck cards'
    /// counters. The caller sets the next phase.
    fn leave_combat(&mut self) {
        let Phase::Combat(combat) = std::mem::replace(&mut self.phase, Phase::GameOver) else {
            unreachable!("a fight ends in the combat phase");
        };
        let potions = combat_belt(&combat);
        let cs = combat.state();
        let player = cs.creature(cs.player.creature);
        self.state.hp = player.hp;
        self.state.max_hp = player.max_hp;
        self.state.gold = cs.player.gold;
        self.state.potions = potions;
        relics::write_back(&mut self.state.relics, &cs.player.relics);
        // Deck versions the fight changed (`TheScythe.BuffFromPlay`, `ImprovementPower`'s
        // upgrades); the deck itself changes only through `CardPileCmd.RemoveFromDeck` (`SwipePower.Steal`), applied below.
        assert_eq!(
            self.state.deck.len(),
            cs.deck.len(),
            "the deck changed in combat"
        );
        for (card, version) in self.state.deck.iter_mut().zip(&cs.deck) {
            card.counter = version.counter;
            // `Goopy` raises its deck version's `Amount`.
            card.enchantment = version.enchantment;
            // `ImprovementPower.AfterCombatEnd` upgrades deck cards (`CardCmd.Upgrade`).
            for _ in card.upgrade_level..version.upgrade_level {
                card.upgrade();
            }
        }
        // A `SpecialCardReward` holds the deck card itself, as it is now.
        let deck = self.state.deck.clone();
        // `CombatRoom.AddExtraReward` during the fight (`RoyaltiesPower`), after the event's.
        // `RewardsSet` populates every reward of its list, so a fixed `GoldReward(amount)`
        // still rolls `NextInt(amount, amount + 1)` on `rewards`.
        let Room::Combat { room_type, .. } = self.state.room else {
            unreachable!("a fight is fought in a combat room");
        };
        self.combat_extra_rewards
            .extend(cs.extra_rewards.iter().map(|r| match *r {
                ExtraReward::Gold(amount) => PendingReward::Gold(amount, amount),
                // `ForRoom` (no `IsFromCombat`), as `PrayerWheel`'s.
                ExtraReward::Card(count) => PendingReward::Card(
                    CardCreation::for_room(odds::CardRarityOddsType::for_room(room_type)),
                    count,
                ),
                ExtraReward::SpecialCard(index) => {
                    PendingReward::Ready(Reward::SpecialCard(deck[index]))
                }
            }));
        self.state.remove_cards(&cs.removed_from_deck);
        self.combat_gold = cs.gold_proportion;
        self.take_back_streams(combat);
    }

    /// The run takes back an ended fight's streams; with events recorded it also keeps the fight
    /// ([`Run::ended_combat`]).
    fn take_back_streams(&mut self, combat: Box<Combat>) {
        if self.record_events {
            self.state.rng.clone_from(&combat.state().rng);
            self.ended_combat = Some(combat);
        } else {
            self.state.rng = combat.into_rng();
        }
    }

    /// A won fight (`CombatManager.EndCombatInternal`): the deck half of `AfterCombatEnd`
    /// (`FishingRod`), then the room's rewards (an event fight's with its extra rewards).
    ///
    /// # Errors
    /// If a relic's victory hook meets content that is not ported (`SwordOfStone` replacing
    /// itself), with the run changed.
    fn win_combat(&mut self) -> Result<(), RunError> {
        let Phase::Combat(combat) = &self.phase else {
            unreachable!("a fight ends in the combat phase");
        };
        let encounter_data = combat.state().encounter_data;
        self.leave_combat();
        let Room::Combat {
            room_type,
            encounter,
        } = self.state.room
        else {
            unreachable!("a fight is fought in a combat room");
        };
        self.after_combat_victory(room_type)?;
        hooks::after_combat_end_on_deck(&mut self.state, room_type);
        let extra = std::mem::take(&mut self.combat_extra_rewards);
        let encounter = Encounter::from_entry(encounter).expect("a fought encounter is ported");
        if let Some(event) = &mut self.suspended_event {
            event.exited = Some(events::ExitedCombat {
                encounter,
                data: encounter_data,
                extra_rewards: extra.clone(),
            });
        }
        if !encounter.should_give_rewards() {
            // `NCombatUi.OnCombatWon` → `ProceedWithoutRewards` →
            // `RunManager.ProceedFromTerminalRewardsScreen`: no rewards screen at all.
            return self.finish_prompt();
        }
        let rewards =
            rewards::generate_combat_rewards(&mut self.state, room_type, self.combat_gold, extra);
        self.phase = Phase::Rewards(rewards);
        Ok(())
    }

    /// `Hook.AfterCombatVictory` for the relics that act on the run (`SwordOfStone`'s
    /// `RelicCmd.Replace`).
    ///
    /// # Errors
    /// If a relic meets content that is not ported, with the run changed.
    ///
    /// # Panics
    /// If a relic opens a prompt.
    fn after_combat_victory(&mut self, room: RoomType) -> Result<(), RunError> {
        // Not the melted relics (`IterateHookListeners`).
        let kinds: Vec<Option<RelicKind>> = self
            .state
            .relics
            .iter()
            .map(|r| (!r.melted).then_some(r.kind))
            .collect();
        let mut ctx = RunCtx::new(self);
        for (me, kind) in kinds.into_iter().enumerate() {
            let Some(kind) = kind else { continue };
            if ctx.state().relics.get(me).is_some_and(|r| r.kind == kind) {
                kind.model()
                    .after_combat_victory_in_run(&mut ctx, me, room)?;
            }
        }
        assert!(ctx.prompt.is_none(), "no relic prompts after a victory");
        Ok(())
    }

    /// `RestSiteOption.Generate`: Heal and Smith (enabled while a card can be upgraded), then
    /// `ModifyRestSiteOptions`.
    fn rest_options(&self) -> Vec<RestOption> {
        let mut options = vec![
            RestOption {
                kind: RestOptionKind::Heal,
                enabled: true,
            },
            RestOption {
                kind: RestOptionKind::Smith,
                enabled: !self.state.upgradable_cards().is_empty(),
            },
        ];
        hooks::modify_rest_site_options(&self.state, &mut options);
        options
    }

    /// After a rest-site option succeeded (`RestSiteSynchronizer.cs:199-218`): unless
    /// `ShouldDisableRemainingRestSiteOptions` says no, the other options go and the player
    /// leaves; else the rest site stays with what is left.
    fn after_rest_option(&mut self, mut rest: RestSite) {
        if hooks::should_disable_remaining_rest_site_options(&self.state) {
            self.leave_room();
        } else {
            rest.used = true;
            self.phase = Phase::RestSite(rest);
        }
    }

    /// `TreasureRoom.EnterInternal` and the chest: `AfterRoomEntered` (`SilverCrucible` counts
    /// the room), then unless `ShouldGenerateTreasure` refuses, a relic from the shared bag and
    /// `rewards.NextInt(42, 53)` gold, ×0.75 truncated from `Poverty` on (`PlayerCmd.GainGold`;
    /// `OneOffSynchronizer.DoTreasureRoomRewards`), then `TryHandleSpoilsMap`.
    fn enter_treasure(&mut self) {
        let s = &mut self.state;
        s.room = Room::Treasure;
        hooks::after_room_entered(s, RoomType::Treasure);
        let relic = hooks::should_generate_treasure(s).then(|| {
            let relic = rewards::roll_treasure_relic(s);
            let gold = s.player_rng[PlayerRngType::Rewards].next_int_range(42, 53);
            s.gain_gold(Decimal::from(rewards::poverty_gold(gold, s.ascension)));
            complete_spoils_quest(s);
            relic
        });
        self.phase = Phase::Treasure(relic);
    }

    /// The shop's actions: each stocked, affordable item (a potion only while `TryToProcure`
    /// would succeed: a free slot and no `ShouldProcurePotion` veto; a failed purchase changes
    /// nothing, `MerchantPotionEntry.OnTryPurchase`), the removal while unused and affordable,
    /// leaving, and the potion uses.
    fn push_shop_actions(&self, shop: &Shop, actions: &mut Vec<RunAction>) {
        let s = &self.state;
        let affordable = |base| shop::price(s, base) <= s.gold;
        let cards = shop
            .cards
            .iter()
            .map(|c| c.card.is_some() && affordable(c.base_cost));
        let relics = shop
            .relics
            .iter()
            .map(|r| r.id.is_some() && affordable(r.base_cost));
        let fits = s.potions.contains(&None) && hooks::should_procure_potion(s);
        let potions = shop
            .potions
            .iter()
            .map(|p| fits && p.id.is_some() && affordable(p.base_cost));
        let pick = |ok: bool, action: RunAction| ok.then_some(action);
        actions.extend(
            cards
                .enumerate()
                .filter_map(|(i, ok)| pick(ok, RunAction::BuyCard(i)))
                .chain(
                    relics
                        .enumerate()
                        .filter_map(|(i, ok)| pick(ok, RunAction::BuyRelic(i))),
                )
                .chain(
                    potions
                        .enumerate()
                        .filter_map(|(i, ok)| pick(ok, RunAction::BuyPotion(i))),
                ),
        );
        if !shop.removal_used
            && affordable(shop.removal_base_cost)
            && s.deck.iter().any(|c| c.is_removable())
        {
            actions.push(RunAction::RemoveCard);
        }
        actions.push(RunAction::Proceed);
        self.push_potion_actions(actions);
    }

    /// A shop action: leaving, or a purchase ([`Run::purchase`]) already checked to be stocked,
    /// affordable and (for a potion) to fit.
    fn shop_action(&mut self, action: &RunAction) -> Result<(), RunError> {
        let Phase::Shop(shop) = &self.phase else {
            return Err(RunError::IllegalAction);
        };
        let mut shop = shop.clone();
        let entry = match *action {
            RunAction::Proceed => {
                self.leave_room();
                return Ok(());
            }
            RunAction::BuyCard(i) => ShopEntry::Card(i),
            RunAction::BuyRelic(i) => ShopEntry::Relic(i),
            RunAction::BuyPotion(i) => ShopEntry::Potion(i),
            RunAction::RemoveCard => ShopEntry::Removal,
            _ => return Err(RunError::IllegalAction),
        };
        let Some(prompt) = self.purchase(&mut shop, entry, false)? else {
            self.phase = Phase::Shop(shop);
            return Ok(());
        };
        self.suspended_shop = Some(shop);
        self.phase = prompt;
        Ok(())
    }

    /// `MerchantEntry.OnTryPurchaseWrapper` (`MerchantEntry.cs:69-96`) of `entry`: the item, the
    /// gold unless `ignore_cost` (then `AfterItemPurchased` sees 0 spent), then the slot is
    /// restocked if `ShouldRefillMerchantEntry` says so, else cleared, then `AfterItemPurchased`
    /// and `UpdateEntries`. An entry out of stock, or a potion the belt refuses, is not bought
    /// (with `ignore_cost`; a player's purchase was checked). A removal opens its deck selection
    /// (its `AfterItemPurchased` follows the pick). A relic that opens a prompt (`AfterObtained`)
    /// finishes its purchase once the prompt ends, as `OnTryPurchase` awaits `RelicCmd.Obtain`:
    /// a `TheCourier` restock rolls on `rewards` after the prompt's own draws. Returns the
    /// prompt to open, if any.
    fn purchase(
        &mut self,
        shop: &mut Shop,
        entry: ShopEntry,
        ignore_cost: bool,
    ) -> Result<Option<Phase>, RunError> {
        let refill = hooks::should_refill_merchant_entry(&self.state);
        let paid = |s: &RunState, base| if ignore_cost { 0 } else { shop::price(s, base) };
        match entry {
            // `CardPileCmd.Add`, then `LoseGold(Cost)`.
            ShopEntry::Card(i) => {
                let entry = &shop.cards[i];
                let Some(card) = entry.card else {
                    return Ok(None);
                };
                let cost = paid(&self.state, entry.base_cost);
                self.state.add_card(card);
                self.state.lose_gold(cost);
                if refill {
                    shop.restock_card(&mut self.state, i);
                } else {
                    shop.cards[i].card = None;
                }
                hooks::after_item_purchased(&mut self.state, cost);
            }
            // `LoseGold(Cost)`, then `RelicCmd.Obtain`.
            ShopEntry::Relic(i) => {
                let entry = &shop.relics[i];
                let Some(id) = entry.id else {
                    return Ok(None);
                };
                let cost = paid(&self.state, entry.base_cost);
                self.state.lose_gold(cost);
                let prompt = self.obtain_relic(id)?;
                if prompt.is_some() {
                    self.relic_purchase_after_prompt = Some((i, cost));
                } else {
                    self.finish_relic_purchase(shop, i, cost);
                }
                return Ok(prompt);
            }
            // `PotionCmd.TryToProcure`, then `LoseGold(Cost)`.
            ShopEntry::Potion(i) => {
                let entry = &shop.potions[i];
                let Some(id) = entry.id else {
                    return Ok(None);
                };
                let cost = paid(&self.state, entry.base_cost);
                if !self.state.procure_potion(id) {
                    return if ignore_cost {
                        Ok(None)
                    } else {
                        Err(RunError::IllegalAction)
                    };
                }
                self.state.lose_gold(cost);
                if refill {
                    shop.restock_potion(&mut self.state, i);
                } else {
                    shop.potions[i].id = None;
                }
                hooks::after_item_purchased(&mut self.state, cost);
            }
            // `DoMerchantCardRemoval`: `FromDeckForRemoval` with one pick and manual
            // confirmation, so it prompts unless no card is removable; cancelable unless a relic
            // buys it.
            ShopEntry::Removal => {
                let options = self.state.removable_cards();
                if shop.removal_used || options.is_empty() {
                    return Ok(None);
                }
                let cost = paid(&self.state, shop.removal_base_cost);
                return Ok(Some(Phase::CardSelect(CardSelect {
                    purpose: SelectPurpose::ShopRemoval {
                        cost,
                        cancelable: !ignore_cost,
                    },
                    options,
                    offered: Vec::new(),
                    count: 1,
                    picked: Vec::new(),
                })));
            }
        }
        shop.update_entries(&self.state);
        Ok(None)
    }

    /// The rest of `OnTryPurchaseWrapper` for relic slot `i` once `RelicCmd.Obtain` is done:
    /// restocked if `ShouldRefillMerchantEntry` says so (asked now), else cleared, then
    /// `AfterItemPurchased` and `UpdateEntries`.
    fn finish_relic_purchase(&mut self, shop: &mut Shop, i: usize, cost: i32) {
        if hooks::should_refill_merchant_entry(&self.state) {
            shop.restock_relic(&mut self.state, i);
        } else {
            shop.relics[i].id = None;
        }
        hooks::after_item_purchased(&mut self.state, cost);
        shop.update_entries(&self.state);
    }

    /// Buys the shop's queued free purchases ([`Shop::free_purchases`], `LordsParasol`) in order,
    /// until one opens a prompt (the rest follow once the shop is back) or none is left.
    fn continue_free_purchases(&mut self) -> Result<(), RunError> {
        loop {
            let Phase::Shop(shop) = &self.phase else {
                return Ok(());
            };
            if shop.free_purchases.is_empty() {
                return Ok(());
            }
            let mut shop = shop.clone();
            let entry = shop.free_purchases.remove(0);
            if let Some(prompt) = self.purchase(&mut shop, entry, true)? {
                self.suspended_shop = Some(shop);
                self.phase = prompt;
                return Ok(());
            }
            self.phase = Phase::Shop(shop);
        }
    }

    /// `Player.CanUseOrRemovePotions` is false: an event that locks the belt
    /// ([`EventKind::locks_potions`]) is on its page or waits behind its prompt.
    fn potions_locked(&self) -> bool {
        self.event().is_some_and(|room| room.kind.locks_potions())
    }

    /// The out-of-combat potion uses: an `AnyTime` potion the engine ports, while the player is
    /// alive and on a screen outside combat and card selections.
    fn potion_actions(&self) -> Vec<RunAction> {
        let mut actions = Vec::new();
        self.push_potion_actions(&mut actions);
        actions
    }

    /// Appends [`Run::potion_actions`] to `actions`.
    fn push_potion_actions(&self, actions: &mut Vec<RunAction>) {
        let screen = matches!(
            self.phase,
            Phase::Ancient(_)
                | Phase::Event(_)
                | Phase::Map
                | Phase::Rewards(_)
                | Phase::RestSite(_)
                | Phase::Treasure(_)
                | Phase::Shop(_)
        );
        if !screen || self.state.hp <= 0 || self.potions_locked() {
            return;
        }
        actions.extend(
            self.state
                .potions_held()
                .filter(|&(_, id)| {
                    potions::combat_potion(id).is_some_and(|k| {
                        k.usage() == PotionUsage::AnyTime && k.usable_out_of_combat(&self.state)
                    })
                })
                .map(|(slot, _)| RunAction::UsePotion(slot)),
        );
    }

    /// `PotionModel.OnUseWrapper` outside combat (`Models/PotionModel.cs:297-354`): the slot
    /// empties, then `OnUse` on the player; `BeforePotionUsed`/`AfterPotionUsed` have no
    /// in-scope listener outside combat.
    fn use_potion(&mut self, slot: usize) -> Result<(), RunError> {
        let id = self.state.potions[slot].ok_or(RunError::IllegalAction)?;
        let kind = potions::combat_potion(id).ok_or(RunError::UnsupportedPotion(id))?;
        self.state.potions[slot] = None;
        kind.use_out_of_combat(&mut self.state)?;
        if self.state.hp <= 0 {
            self.phase = Phase::GameOver;
        }
        Ok(())
    }
}

/// The phase of a prompt a relic opened outside an event.
fn prompt_phase(prompt: Prompt) -> Phase {
    match prompt {
        Prompt::Rewards(rewards) => Phase::Rewards(rewards),
        Prompt::Select(select) => Phase::CardSelect(select),
        Prompt::Bundles(bundles) => Phase::Bundles(bundles),
        Prompt::Combat { .. } => unreachable!("only events start fights"),
    }
}

/// `Player.CreateForNewRun`'s starter deck, then `AscensionManager.ApplyEffectsTo`: Ascender's
/// Bane joins (silently) from `AscendersBane` on.
/// `OneOffSynchronizer.TryHandleSpoilsMap` (`OneOffSynchronizer.cs:145-167`): on the point with a
/// `SpoilsMap` quest marker, every `SpoilsMap` of the deck completes its quest
/// (`SpoilsMap.OnQuestComplete`): its `Gold` (600) through `PlayerCmd.GainGold`, then
/// `CardPileCmd.RemoveFromDeck` (whose `BeforeCardRemoved` takes the marker off). The quest stat
/// (`PlayerCmd.CompleteQuest`) is not modeled.
fn complete_spoils_quest(s: &mut RunState) {
    if s.spoils_quest.is_none() || s.coord() != s.spoils_quest {
        return;
    }
    let count = s.deck.iter().filter(|c| c.id == SPOILS_MAP).count();
    for _ in 0..count {
        s.gain_gold(Decimal::from(SPOILS_MAP_GOLD));
        if let Some(i) = s.deck.iter().position(|c| c.id == SPOILS_MAP) {
            s.remove_card(i);
        }
    }
}

/// `SpoilsMap`'s `ModelId.Entry`.
pub(crate) const SPOILS_MAP: &str = "SPOILS_MAP";
/// `SpoilsMap`'s `GoldVar(600)`.
const SPOILS_MAP_GOLD: i32 = 600;

fn starting_deck(character: Character, ascension: u8) -> Vec<RunCard> {
    let mut deck = cards::starter_deck(character);
    if AscensionLevel::AscendersBane.is_active(ascension) {
        deck.push(RunCard::new("ASCENDERS_BANE"));
    }
    deck
}

/// The room type of an encounter (`EncounterModel.RoomType`).
const fn encounter_room_type(room: EncounterRoom) -> RoomType {
    match room {
        EncounterRoom::Monster => RoomType::Monster,
        EncounterRoom::Elite => RoomType::Elite,
        EncounterRoom::Boss => RoomType::Boss,
    }
}

/// The combat's belt as run potion ids, by slot (ported and unported potions).
fn combat_belt(combat: &Combat) -> Vec<Option<&'static str>> {
    let player = &combat.state().player;
    player
        .potions
        .iter()
        .zip(&player.unported_potions)
        .map(|(ported, unported)| ported.map(|p| potions::potion_id(p.kind)).or(*unported))
        .collect()
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_act_flow;
#[cfg(test)]
mod tests_ancient;
#[cfg(test)]
mod tests_aud3;
#[cfg(test)]
mod tests_c25;
#[cfg(test)]
mod tests_c26;
#[cfg(test)]
mod tests_c28;
#[cfg(test)]
mod tests_cq1;
#[cfg(test)]
mod tests_map_hooks;
#[cfg(test)]
mod tests_rb;
#[cfg(test)]
mod tests_xan1;
#[cfg(test)]
mod tests_xan2;
#[cfg(test)]
mod tests_xev1;
#[cfg(test)]
mod tests_xev3;
#[cfg(test)]
mod tests_xev4;
