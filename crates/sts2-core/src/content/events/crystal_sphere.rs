//! `CrystalSphere` (`Models.Events/CrystalSphere.cs`): pay 51-99 gold for 3 divinations, or take
//! a `Debt` for 6, on the Crystal Sphere minigame (`Events.Custom.CrystalSphereEvent/`). Shared,
//! Act 2 and 3.
//!
//! The minigame is an 11×11 fogged grid hiding 15 items, placed on the event `Rng` when an option
//! is chosen. Each divination clears one cell (small tool) or the 3×3 square around it (big
//! tool); an item whose cells are all clear is revealed. Once the divinations are spent, the
//! revealed items' rewards are offered, rolled on the event `Rng`. The engine shows the grid as
//! event pages: one option per hidden cell and tool (only hidden cells are clickable,
//! `NCrystalSphereCell.MouseFilter`), with made-up `CRYSTAL_SPHERE.minigame.*` text keys. The
//! headless referee cannot play it (`PlayMinigame` awaits the screen), so it is unit-tested only.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::rng::Rng;
use crate::run::odds::{CardRarity, PotionRarity};
use crate::run::potions::potion_options;
use crate::run::rewards::{
    CardCreation, CardFilter, CardPool, create_card_reward_on, pull_relic_from_front_on,
};
use crate::run::{CardReward, Reward, RunCard, RunError, RunState};

/// `_uncoverFutureCost`, plus `Rng.NextInt(1, 50)` in `CalculateVars`.
const UNCOVER_FUTURE_COST: i32 = 50;
/// `_uncoverFutureRandomMin`, `_uncoverFutureRandomMax`.
const UNCOVER_FUTURE_RANDOM: (i32, i32) = (1, 50);
/// `_uncoverFutureProphesizeCount`.
const UNCOVER_FUTURE_DIVINATIONS: u32 = 3;
/// `_paymentPlanCount`.
const PAYMENT_PLAN_DIVINATIONS: u32 = 6;
/// `_defaultWidth`, `_defaultHeight`.
const SIZE: i32 = 11;
/// `PopulateItems` is tried at most this many times.
const PLACEMENT_TRIES: u32 = 10;
/// The cards a card reward offers (`new CardReward(options, 3, owner)`).
const REWARD_CARDS: usize = 3;

pub struct CrystalSphere;

/// A `CrystalSphereItem`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// `CrystalSphereRelic`: a `RelicReward` on the event `Rng`.
    Relic,
    /// `CrystalSpherePotion`: a potion of the rarity.
    Potion(PotionRarity),
    /// `CrystalSphereCardReward`: 3 character cards of the rarity.
    CardReward(CardRarity),
    /// `CrystalSphereCurse`: a `Doubt` joins the deck when it is revealed.
    Curse,
    /// `CrystalSphereGold`: 10 gold, or 30 if big.
    Gold { big: bool },
}

impl ItemKind {
    /// `Size` (x, y).
    const fn size(self) -> (i32, i32) {
        match self {
            Self::Relic => (4, 4),
            Self::Potion(PotionRarity::Rare) | Self::CardReward(_) | Self::Curse => (2, 2),
            Self::Potion(_) => (1, 3),
            Self::Gold { big: false } => (1, 1),
            Self::Gold { big: true } => (2, 1),
        }
    }
}

/// An item on the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Item {
    pub kind: ItemKind,
    /// `Position` (0, 0 while it is not placed).
    pub x: i32,
    pub y: i32,
    /// How many times `OnItemRevealed` is subscribed to its `Revealed`: every `PopulateItems`
    /// try subscribes all the items so far again, so an item of an earlier try is revealed
    /// (and rewarded) more than once.
    pub subscriptions: u8,
}

/// `CrystalSphereMinigame`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Minigame {
    /// `cells[x, y].IsHidden`, at `x * 11 + y`.
    pub hidden: Vec<bool>,
    /// `cells[x, y].Item`, an index into `items`.
    pub cell_items: Vec<Option<usize>>,
    /// `_items`.
    pub items: Vec<Item>,
    /// `_revealed`: item indices, in reveal order.
    pub revealed: Vec<usize>,
    /// `DivinationCount`.
    pub divinations: u32,
}

/// The index of cell (x, y).
fn cell(x: i32, y: i32) -> usize {
    usize::try_from(x * SIZE + y).expect("a cell of the grid")
}

/// `GetHorizontalCells`.
fn horizontal(x: i32, y: i32) -> Vec<(i32, i32)> {
    [-1, 1]
        .into_iter()
        .map(|d| (x + d, y))
        .filter(|&(nx, _)| (0..SIZE).contains(&nx))
        .collect()
}

/// `GetVerticalCells`.
fn vertical(x: i32, y: i32) -> Vec<(i32, i32)> {
    [-1, 1]
        .into_iter()
        .map(|d| (x, y + d))
        .filter(|&(_, ny)| (0..SIZE).contains(&ny))
        .collect()
}

/// `GetAdjacentCells`: horizontal, vertical, diagonal neighbours, then the cell itself.
fn adjacent(x: i32, y: i32) -> Vec<(i32, i32)> {
    let mut cells = horizontal(x, y);
    cells.extend(vertical(x, y));
    for dx in [-1, 1] {
        for dy in [-1, 1] {
            let (nx, ny) = (x + dx, y + dy);
            if (0..SIZE).contains(&nx) && (0..SIZE).contains(&ny) {
                cells.push((nx, ny));
            }
        }
    }
    cells.push((x, y));
    cells
}

impl Minigame {
    /// `new CrystalSphereMinigame(owner, rng, divinationCount)`: the corners cleared (each
    /// corner and its neighbours, expanded twice), then the items placed.
    #[must_use]
    pub fn new(rng: &mut Rng, divinations: u32) -> Self {
        let n = cell(SIZE, 0);
        let mut game = Self {
            hidden: vec![true; n],
            cell_items: vec![None; n],
            items: Vec::new(),
            revealed: Vec::new(),
            divinations,
        };
        let last = SIZE - 1;
        let mut cleared = vec![(0, 0), (last, 0), (last, last), (0, last)];
        for _ in 0..2 {
            let h: Vec<(i32, i32)> = cleared
                .iter()
                .flat_map(|&(x, y)| horizontal(x, y))
                .collect();
            let v: Vec<(i32, i32)> = cleared.iter().flat_map(|&(x, y)| vertical(x, y)).collect();
            cleared.extend(h);
            cleared.extend(v);
        }
        for (x, y) in cleared {
            game.hidden[cell(x, y)] = false;
        }
        for _ in 0..PLACEMENT_TRIES {
            if game.populate_items(rng) {
                break;
            }
        }
        game
    }

    /// `PopulateItems`: the 15 items in order; after a placement fails the rest are added
    /// unplaced (`flag && PlaceItem`). Every item so far gets one more subscription.
    fn populate_items(&mut self, rng: &mut Rng) -> bool {
        let mut kinds = vec![
            ItemKind::Relic,
            ItemKind::Potion(PotionRarity::Common),
            ItemKind::Potion(PotionRarity::Common),
            ItemKind::Potion(PotionRarity::Rare),
            ItemKind::CardReward(CardRarity::Common),
            ItemKind::CardReward(CardRarity::Uncommon),
            ItemKind::CardReward(CardRarity::Rare),
            ItemKind::Curse,
        ];
        kinds.extend([ItemKind::Gold { big: false }; 5]);
        kinds.extend([ItemKind::Gold { big: true }; 2]);
        let mut placed = true;
        for kind in kinds {
            let index = self.items.len();
            self.items.push(Item {
                kind,
                x: 0,
                y: 0,
                subscriptions: 0,
            });
            placed = placed && self.place_item(index, rng);
        }
        for item in &mut self.items {
            item.subscriptions += 1;
        }
        placed
    }

    /// `PlaceItem`: `Rng.NextItem` of the positions (x, then y) where the item fits on hidden,
    /// empty cells.
    fn place_item(&mut self, index: usize, rng: &mut Rng) -> bool {
        let (w, h) = self.items[index].kind.size();
        let mut positions = Vec::new();
        for x in 0..SIZE {
            for y in 0..SIZE {
                let fits = (0..w).all(|i| {
                    (0..h).all(|j| {
                        let (cx, cy) = (x + i, y + j);
                        cx < SIZE
                            && cy < SIZE
                            && self.hidden[cell(cx, cy)]
                            && self.cell_items[cell(cx, cy)].is_none()
                    })
                });
                if fits {
                    positions.push((x, y));
                }
            }
        }
        let Some(&(x, y)) = rng.next_item(&positions) else {
            return false;
        };
        let item = &mut self.items[index];
        item.x = x;
        item.y = y;
        for i in 0..w {
            for j in 0..h {
                self.cell_items[cell(x + i, y + j)] = Some(index);
            }
        }
        true
    }

    /// `CellClicked`: one divination; the small tool clears the cell, the big one its
    /// `GetAdjacentCells`. Returns the curses revealed (each adds a `Doubt` at once).
    fn click(&mut self, x: i32, y: i32, big: bool) -> usize {
        self.divinations -= 1;
        let cells = if big { adjacent(x, y) } else { vec![(x, y)] };
        let mut curses = 0;
        for (cx, cy) in cells {
            if !self.hidden[cell(cx, cy)] {
                continue;
            }
            self.hidden[cell(cx, cy)] = false;
            let Some(index) = self.cell_items[cell(cx, cy)] else {
                continue;
            };
            let item = self.items[index];
            let (w, h) = item.kind.size();
            let clear = (0..w).all(|i| (0..h).all(|j| !self.hidden[cell(item.x + i, item.y + j)]));
            if clear {
                for _ in 0..item.subscriptions {
                    self.revealed.push(index);
                }
                if item.kind == ItemKind::Curse {
                    curses += 1;
                }
            }
        }
        curses
    }

    /// The page: a big-tool option for every hidden cell, then a small-tool one.
    fn options(&self) -> Vec<EventOption> {
        let hidden: Vec<usize> = (0..self.hidden.len()).filter(|&i| self.hidden[i]).collect();
        let mut options = Vec::with_capacity(hidden.len() * 2);
        for (tool, tag) in [("BIG", "BIG"), ("SMALL", "SMALL")] {
            for &i in &hidden {
                let key = format!("CRYSTAL_SPHERE.minigame.{tool}");
                options.push(EventOption::with_arg(key, tag, i));
            }
        }
        options
    }
}

/// `OfferCrystalSphereRewards`: each revealed item's `ToReward` in reveal order (a potion picks
/// on the event `Rng` there), then `OfferCustom` populates them in order (cards and the relic on
/// the event `Rng`).
fn offer_rewards(ctx: &mut EventCtx<'_>, revealed: &[ItemKind]) {
    enum Built {
        Ready(Reward),
        Cards(CardRarity),
        Relic,
    }
    let character = ctx.state().character;
    let mut rng = ctx.rng.clone();
    let mut built = Vec::new();
    for &kind in revealed {
        match kind {
            ItemKind::Gold { big } => {
                built.push(Built::Ready(Reward::Gold(if big { 30 } else { 10 })));
            }
            ItemKind::Potion(rarity) => {
                let potions: Vec<&'static str> = potion_options(character)
                    .into_iter()
                    .filter(|&(_, r)| r == rarity)
                    .map(|(id, _)| id)
                    .collect();
                let potion = *rng.next_item(&potions).expect("a potion of the rarity");
                built.push(Built::Ready(Reward::Potion(potion)));
            }
            ItemKind::CardReward(rarity) => built.push(Built::Cards(rarity)),
            ItemKind::Relic => built.push(Built::Relic),
            ItemKind::Curse => {}
        }
    }
    let s: &mut RunState = ctx.state();
    let rewards: Vec<Reward> = built
        .into_iter()
        .map(|b| match b {
            Built::Ready(reward) => reward,
            Built::Cards(rarity) => {
                // `CardCreationOptions(character pool, Other, Uniform, rarity filter)`: the
                // upgrade roll stays on.
                let mut options = CardCreation::non_combat_uniform(
                    CardPool::Character,
                    CardFilter::Rarity(rarity),
                );
                options.upgrade_roll = true;
                let options = options.as_card_reward();
                let cards = create_card_reward_on(s, REWARD_CARDS, options, Some(&mut rng));
                let mut reward = CardReward::with_cards(cards, options);
                reward.event_rng = true;
                Reward::Card(reward)
            }
            Built::Relic => Reward::Relic(pull_relic_from_front_on(s, &mut rng)),
        })
        .collect();
    *ctx.rng = rng;
    ctx.offer_rewards(rewards);
}

impl EventModel for CrystalSphere {
    /// 100 gold and not Act 1 (`CrystalSphere.cs:49-56`).
    fn is_allowed(&self, s: &RunState) -> bool {
        s.gold >= 100 && s.act_index > 0
    }

    /// `CalculateVars`: `UncoverFutureCost += Rng.NextInt(1, 50)`.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let (min, max) = UNCOVER_FUTURE_RANDOM;
        let cost = UNCOVER_FUTURE_COST + ctx.rng.next_int_range(min, max);
        *ctx.data = EventData::Amount(Decimal::from(cost));
        Ok(vec![
            EventOption::new(
                "CRYSTAL_SPHERE.pages.INITIAL.options.UNCOVER_FUTURE",
                "UNCOVER_FUTURE",
            ),
            EventOption::new(
                "CRYSTAL_SPHERE.pages.INITIAL.options.PAYMENT_PLAN",
                "PAYMENT_PLAN",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let game = match action.tag {
            "UNCOVER_FUTURE" => {
                let EventData::Amount(cost) = *ctx.data else {
                    unreachable!("the cost is set in begin");
                };
                ctx.state().lose_gold(cost.to_int_truncating());
                Box::new(Minigame::new(ctx.rng, UNCOVER_FUTURE_DIVINATIONS))
            }
            "PAYMENT_PLAN" => {
                ctx.state().add_card(RunCard::new("DEBT"));
                Box::new(Minigame::new(ctx.rng, PAYMENT_PLAN_DIVINATIONS))
            }
            _ => {
                let EventData::CrystalSphere(mut game) = ctx.data.clone() else {
                    unreachable!("the minigame is set by the first option");
                };
                let index = i32::try_from(action.arg).expect("a cell of the grid");
                let curses = game.click(index / SIZE, index % SIZE, action.tag == "BIG");
                for _ in 0..curses {
                    // `CardPileCmd.AddCurseToDeck<Doubt>`.
                    ctx.state().add_card(RunCard::new("DOUBT"));
                }
                if game.divinations == 0 {
                    let revealed: Vec<ItemKind> =
                        game.revealed.iter().map(|&i| game.items[i].kind).collect();
                    *ctx.data = EventData::CrystalSphere(game);
                    offer_rewards(ctx, &revealed);
                    return Ok(Page::finished("CRYSTAL_SPHERE", "FINISH"));
                }
                game
            }
        };
        let options = game.options();
        *ctx.data = EventData::CrystalSphere(game);
        Ok(Page::Next(options))
    }
}
