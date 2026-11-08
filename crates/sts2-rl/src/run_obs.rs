//! The run env's observation (`docs/specs/rl-2x-plan.md`, stage 9).
//!
//! Little-endian plain integers, no floats, no one-hot; unused slots and the sections of
//! decisions that are not pending are zero. Content is written as [`crate::vocab`] ids (0 an
//! empty slot, 1 content the vocabulary does not list).
//!
//! Sections, at these offsets ([`RUN_OBS_BYTES`] in all):
//! - [`RUN_HEADER`] bytes from 0: character, ascension, act index (`u8` each), phase (`u8`,
//!   [`phase_code`]), act (`u16`), floor in the act, total floor (`u8`), HP, max HP, gold
//!   (`i16`), max energy, belt slots (`u8`), deck size (`u16`), relic count, shop removals used
//!   (`u8`), the act's boss and second boss encounters (`u16`), the current map column and row
//!   (`u8`, 255 before the act's first point), the room's type (`u8`, `RoomType` index + 1 in a
//!   combat room, else 0), 0, and the room's encounter or event (`u16`).
//!   During a fight HP and the belt in this header are as of its start; the combat section has
//!   the live HP and the belt section the live belt.
//! - [`DECK_AT`]: [`DECK`] × (card `u16`, upgrade `u8`, flags `u8`: enchanted in bit 0, the
//!   enchantment's `EnchantmentKind` index + 1 in bits 1-7).
//! - [`RELICS_AT`]: [`RELICS`] × (relic `u16`, flags `u8`: wax, melted from bit 0, 0).
//! - [`BELT_AT`]: [`BELT`] × potion `u16`, live during a fight.
//! - [`MAP_AT`]: the act's map, [`MAP_POINTS`] × (column, row, type + 1, flags: visited,
//!   current, a legal next point from bit 0, children as a column bitmask; `u8` each): the
//!   start, the points in row order, the boss, the second boss.
//! - [`ANCIENT_AT`]: the ancient (`u16`), its options × [`OPTIONS`] as (relic `u16`, locked
//!   `u8`).
//! - [`EVENT_AT`]: the event (`u16`), its page's options × [`OPTIONS`] as (text key `u16`:
//!   FNV-1a of the option's text key folded to 16 bits, never 0; enabled `u8`).
//! - [`REWARDS_AT`]: [`REWARDS`] × [`REWARD_BYTES`]: kind (`u8`: 1 gold, 2 potion, 3 relic,
//!   4 wax relic, 5 cards, 6 a given card), flags (`u8`: can reroll), value (`u16`: the gold, or
//!   the potion, relic or given card), the given card's upgrade (`u8`), 0, then a card reward's
//!   cards × [`REWARD_CARDS`] as (card `u16`, upgrade `u8`).
//! - [`SELECT_AT`]: a deck card selection: purpose (`u8`, [`select_code`]), min, count, picked,
//!   cancelable, 0, 0, 0 (`u8`), options × [`SELECT_OPTIONS`] as (card `u16`, upgrade `u8`, deck
//!   index `u8` (255 for a card not in the deck), flags `u8`: picked, enchanted from bit 0).
//! - [`BUNDLES_AT`]: [`BUNDLES`] × [`BUNDLE_CARDS`] × (card `u16`, upgrade `u8`).
//! - [`REST_AT`]: the rest site's options × [`OPTIONS`] as (`RestOptionKind` + 1, enabled).
//! - [`TREASURE_AT`]: the chest's relic (`u16`).
//! - [`SHOP_AT`]: cards × [`SHOP_CARDS`] as (card `u16`, upgrade, on sale (`u8`), price
//!   `i16`), relics × [`SHOP_RELICS`] and potions × [`SHOP_POTIONS`] as (id `u16`, price `i16`),
//!   the removal's price (`i16`), removal used (`u8`), 0. A sold entry is zero.
//! - [`COMBAT_AT`]: during a fight the combat observation ([`crate::obs`], [`OBS_BYTES`]),
//!   then from [`COMBAT_STATIC_AT`] its static part ([`STATIC_BYTES`]).
//!
//! Nothing hidden is read: not the RNG streams, the odds, the relic bags, the acts' encounter
//! and event lists or an event's own `Rng` (`tests/leak.rs` reseeds and reorders them all).

use sts2_core::Combat;
use sts2_core::combat::CombatState;
use sts2_core::content::ancients::AncientOption;
use sts2_core::run::{
    CardSelect, MapCoord, Reward, Room, RunCard, RunDecision, RunState, SelectPurpose, shop,
};

use crate::obs::{OBS_BYTES, Observer, STATIC_BYTES, Writer, observe_static};
use crate::run_action::{
    BUNDLE_SLOTS, OPTION_SLOTS, REWARD_CARD_SLOTS, REWARD_SLOTS, SELECT_SLOTS, SHOP_CARD_SLOTS,
    SHOP_POTION_SLOTS, SHOP_RELIC_SLOTS,
};
use crate::vocab::{self, FNV_BASIS, fnv};

/// Deck slots.
pub const DECK: usize = 80;
/// Relic slots.
pub const RELICS: usize = 32;
/// Belt slots.
pub const BELT: usize = crate::action::POTION_SLOTS;
/// Map point slots.
pub const MAP_POINTS: usize = 96;
/// Options of an ancient, an event page or a rest site.
pub const OPTIONS: usize = OPTION_SLOTS;
/// Rewards of a rewards screen.
pub const REWARDS: usize = REWARD_SLOTS;
/// Cards of a card reward.
pub const REWARD_CARDS: usize = REWARD_CARD_SLOTS;
/// Options of a deck selection.
pub const SELECT_OPTIONS: usize = SELECT_SLOTS;
/// Bundles, and cards per bundle.
pub const BUNDLES: usize = BUNDLE_SLOTS;
pub const BUNDLE_CARDS: usize = 4;
/// Shop entries.
pub const SHOP_CARDS: usize = SHOP_CARD_SLOTS;
pub const SHOP_RELICS: usize = SHOP_RELIC_SLOTS;
pub const SHOP_POTIONS: usize = SHOP_POTION_SLOTS;

/// Bytes of the header.
pub const RUN_HEADER: usize = 32;
/// Bytes per map point.
pub const MAP_POINT_BYTES: usize = 5;
/// Bytes per reward.
pub const REWARD_BYTES: usize = 6 + REWARD_CARDS * 3;
/// Bytes per selection option.
pub const SELECT_OPTION_BYTES: usize = 5;

pub const DECK_AT: usize = RUN_HEADER;
pub const RELICS_AT: usize = DECK_AT + DECK * 4;
pub const BELT_AT: usize = RELICS_AT + RELICS * 4;
pub const MAP_AT: usize = BELT_AT + BELT * 2;
pub const ANCIENT_AT: usize = MAP_AT + MAP_POINTS * MAP_POINT_BYTES;
pub const EVENT_AT: usize = ANCIENT_AT + 2 + OPTIONS * 3;
pub const REWARDS_AT: usize = EVENT_AT + 2 + OPTIONS * 3;
pub const SELECT_AT: usize = REWARDS_AT + REWARDS * REWARD_BYTES;
pub const BUNDLES_AT: usize = SELECT_AT + 8 + SELECT_OPTIONS * SELECT_OPTION_BYTES;
pub const REST_AT: usize = BUNDLES_AT + BUNDLES * BUNDLE_CARDS * 3;
pub const TREASURE_AT: usize = REST_AT + OPTIONS * 2;
pub const SHOP_AT: usize = TREASURE_AT + 2;
pub const COMBAT_AT: usize = SHOP_AT + SHOP_CARDS * 6 + (SHOP_RELICS + SHOP_POTIONS) * 4 + 4;
pub const COMBAT_STATIC_AT: usize = COMBAT_AT + OBS_BYTES;
/// Bytes of the run observation.
pub const RUN_OBS_BYTES: usize = COMBAT_STATIC_AT + STATIC_BYTES;

/// The phase code of a pending decision.
///
/// 0 ancient, 1 event, 2 map, 3 combat, 4 rewards, 5 deck selection, 6 bundles, 7 rest site, 8
/// treasure, 9 shop, 10 a stub room, 11 stopped at unported content, 12 game over, 13 victory.
#[must_use]
pub const fn phase_code(decision: &RunDecision<'_>) -> u8 {
    match decision {
        RunDecision::Ancient(_) => 0,
        RunDecision::Event(_) => 1,
        RunDecision::Map(_) => 2,
        RunDecision::Combat(_) => 3,
        RunDecision::Rewards(_) => 4,
        RunDecision::CardSelect(_) => 5,
        RunDecision::Bundles(_) => 6,
        RunDecision::RestSite(_) => 7,
        RunDecision::Treasure(_) => 8,
        RunDecision::Shop(_) => 9,
        RunDecision::Unsupported(_) => 10,
        RunDecision::Stopped(_) => 11,
        RunDecision::GameOver => 12,
        RunDecision::Victory => 13,
    }
}

/// A selection purpose's code: `SelectPurpose`'s variants in declaration order from 1.
#[must_use]
pub const fn select_code(purpose: SelectPurpose) -> u8 {
    match purpose {
        SelectPurpose::Upgrade => 1,
        SelectPurpose::Smith => 2,
        SelectPurpose::Cook => 3,
        SelectPurpose::Remove { .. } => 4,
        SelectPurpose::AddToDeck => 5,
        SelectPurpose::ShopRemoval { .. } => 6,
        SelectPurpose::Enchant { .. } => 7,
        SelectPurpose::EnchantUpTo { .. } => 8,
        SelectPurpose::Clone => 9,
        SelectPurpose::Event { .. } => 10,
        SelectPurpose::ChooseACard { .. } => 11,
        SelectPurpose::TransformRandom => 12,
        SelectPurpose::Relic { .. } => 13,
        SelectPurpose::RelicUpTo { .. } => 14,
        SelectPurpose::TransformRandomUpgraded => 15,
        SelectPurpose::AddToDeckUpTo => 16,
    }
}

/// An event option's text key as a 16-bit id: FNV-1a folded, never 0.
#[must_use]
pub fn text_key_id(key: &str) -> u16 {
    let h = fnv(FNV_BASIS, key.as_bytes());
    let folded = (h ^ (h >> 16) ^ (h >> 32) ^ (h >> 48)).to_le_bytes();
    u16::from_le_bytes([folded[0], folded[1]]).max(1)
}

/// A fight as the run observation reads it: the fight, and the state to observe (the fight's
/// own, or a test's changed copy).
#[derive(Clone, Copy, Debug)]
pub struct CombatView<'a> {
    pub combat: &'a Combat,
    pub state: &'a CombatState,
}

/// One env's run observation writer, with the combat observer it uses during fights.
#[derive(Debug)]
pub struct RunObserver {
    combat: Observer,
    /// The last observation was of a fight (a new fight resets the combat caches).
    in_combat: bool,
}

impl RunObserver {
    /// An observer; without `intents` the combat section's intents stay zero.
    #[must_use]
    pub fn new(intents: bool) -> Self {
        Self {
            combat: Observer::new(intents),
            in_combat: false,
        }
    }

    /// Starts a new episode.
    pub fn reset(&mut self) {
        self.combat.reset();
        self.in_combat = false;
    }

    /// Writes `run`'s observation into `out` ([`RUN_OBS_BYTES`]).
    ///
    /// # Panics
    /// If `out` is shorter than [`RUN_OBS_BYTES`].
    pub fn observe(&mut self, run: &sts2_core::run::Run, out: &mut [u8]) {
        let combat = run.combat().map(|combat| CombatView {
            combat,
            state: combat.state(),
        });
        self.observe_parts(run.state(), &run.pending_decision(), combat, out);
    }

    /// [`RunObserver::observe`] of a run with state `s`, pending `decision` and the fight
    /// `combat`, so a test can observe a state it changed.
    ///
    /// # Panics
    /// If `out` is shorter than [`RUN_OBS_BYTES`].
    pub fn observe_parts(
        &mut self,
        s: &RunState,
        decision: &RunDecision<'_>,
        combat: Option<CombatView<'_>>,
        out: &mut [u8],
    ) {
        let out = &mut out[..RUN_OBS_BYTES];
        {
            let mut w = Writer::new(&mut out[..COMBAT_AT]);
            header(s, decision, &mut w);
            deck(s, &mut w);
            relics(s, &mut w);
            belt(s, combat.map(|c| c.state), &mut w);
            map(s, decision, &mut w);
            pending(s, decision, &mut w);
        }
        let Some(view) = combat else {
            self.in_combat = false;
            out[COMBAT_AT..].fill(0);
            return;
        };
        if !self.in_combat {
            self.combat.reset();
            self.in_combat = true;
        }
        let (dynamic, statics) = out[COMBAT_AT..].split_at_mut(OBS_BYTES);
        self.combat.observe(view.combat, dynamic);
        observe_static(view.combat, statics);
    }
}

/// A run card as (card, upgrade); a card the engine does not port is looked up by its id.
fn card(w: &mut Writer<'_>, card: &RunCard) {
    w.u16(
        card.kind()
            .map_or_else(|| vocab::card_id(card.id), vocab::card),
    );
    w.u8(card.upgrade_level);
}

fn header(s: &RunState, decision: &RunDecision<'_>, w: &mut Writer<'_>) {
    w.u8(s.character as u8);
    w.u8(s.ascension);
    w.u8(s.act_index);
    w.u8(phase_code(decision));
    w.u16(vocab::act(s.act().act.id));
    w.u8(s.act_floor);
    w.u8(s.total_floor());
    w.i16(s.hp);
    w.i16(s.max_hp);
    w.i16(s.gold);
    w.u8(s.max_energy);
    w.u8(s.potions.len());
    w.u16(s.deck.len());
    w.u8(s.relics.len());
    w.u8(s.card_shop_removals_used);
    w.u16(vocab::encounter_id(s.act().boss.id));
    w.u16(
        s.act()
            .second_boss
            .map_or(vocab::PAD, |b| vocab::encounter_id(b.id)),
    );
    let coord = s.coord();
    w.u8(coord.map_or(u8::MAX, |c| u8::try_from(c.col).unwrap_or(u8::MAX)));
    w.u8(coord.map_or(u8::MAX, |c| u8::try_from(c.row).unwrap_or(u8::MAX)));
    let (room_type, model) = match &s.room {
        Room::Combat {
            room_type,
            encounter,
        } => (room_type.enum_index() + 1, vocab::encounter_id(encounter)),
        Room::Event { id } => (0, vocab::event_id(id)),
        _ => (0, vocab::PAD),
    };
    w.u8(room_type);
    w.u8(0);
    w.u16(model);
}

fn deck(s: &RunState, w: &mut Writer<'_>) {
    w.seek(DECK_AT);
    for c in s.deck.iter().take(DECK) {
        card(w, c);
        w.u8(c.enchantment.map_or(0, |e| 1 | (e.kind as u8 + 1) << 1));
    }
}

fn relics(s: &RunState, w: &mut Writer<'_>) {
    w.seek(RELICS_AT);
    for relic in s.relics.iter().take(RELICS) {
        w.u16(vocab::relic(relic.kind));
        w.u8(u8::from(relic.wax) | u8::from(relic.melted) << 1);
        w.u8(0);
    }
}

/// The belt: the fight's live one during a fight, else the run's.
fn belt(s: &RunState, combat: Option<&CombatState>, w: &mut Writer<'_>) {
    w.seek(BELT_AT);
    if let Some(c) = combat {
        let p = &c.player;
        for slot in 0..p.potions.len().min(BELT) {
            let code = match (&p.potions[slot], p.unported_potions.get(slot)) {
                (Some(potion), _) => vocab::potion(potion.kind),
                (None, Some(Some(id))) => vocab::potion_id(id),
                _ => vocab::PAD,
            };
            w.u16(code);
        }
        return;
    }
    for potion in s.potions.iter().take(BELT) {
        w.u16(potion.map_or(vocab::PAD, vocab::potion_id));
    }
}

/// Map rows a [`Grid`] holds (an act has about 17).
const GRID_ROWS: usize = 64;

/// A set of map points: a column bitmask per row.
struct Grid([u8; GRID_ROWS]);

/// The bit of column `col` (0 past the eighth column).
fn column_bit(col: usize) -> u8 {
    u32::try_from(col)
        .ok()
        .and_then(|col| 1_u8.checked_shl(col))
        .unwrap_or(0)
}

impl Grid {
    fn of<'a>(coords: impl IntoIterator<Item = &'a MapCoord>) -> Self {
        let mut rows = [0; GRID_ROWS];
        for c in coords {
            if let Some(row) = rows.get_mut(c.row) {
                *row |= column_bit(c.col);
            }
        }
        Self(rows)
    }

    fn contains(&self, c: MapCoord) -> bool {
        self.0
            .get(c.row)
            .is_some_and(|row| row & column_bit(c.col) != 0)
    }
}

fn map(s: &RunState, decision: &RunDecision<'_>, w: &mut Writer<'_>) {
    w.seek(MAP_AT);
    let current = s.coord();
    let next = match decision {
        RunDecision::Map(next) => Grid::of(next),
        _ => Grid::of(&[]),
    };
    let visited = Grid::of(&s.visited);
    let m = &s.map;
    let points = std::iter::once(m.start())
        .chain(m.points())
        .chain(std::iter::once(m.boss()))
        .chain(m.second_boss());
    for p in points.take(MAP_POINTS) {
        w.u8(p.coord.col);
        w.u8(p.coord.row);
        w.u8(p.kind as u8 + 1);
        w.u8(u8::from(visited.contains(p.coord))
            | u8::from(current == Some(p.coord)) << 1
            | u8::from(next.contains(p.coord)) << 2);
        w.u8(p
            .children
            .iter()
            .fold(0, |bits, c| bits | column_bit(c.col)));
    }
}

/// The pending decision's section.
fn pending(s: &RunState, decision: &RunDecision<'_>, w: &mut Writer<'_>) {
    match decision {
        RunDecision::Ancient(room) => {
            w.seek(ANCIENT_AT);
            w.u16(vocab::ancient(room.kind));
            for option in room.options.iter().take(OPTIONS) {
                w.u16(option.relic_id().map_or(vocab::PAD, vocab::relic_id));
                w.u8(u8::from(matches!(option, AncientOption::Locked(_))));
            }
        }
        RunDecision::Event(room) => {
            w.seek(EVENT_AT);
            w.u16(vocab::event(room.kind));
            for option in room.options.iter().take(OPTIONS) {
                w.u16(text_key_id(&option.text_key));
                w.u8(u8::from(option.action.is_some()));
            }
        }
        RunDecision::Rewards(rewards) => {
            for (i, reward) in rewards.iter().take(REWARDS).enumerate() {
                w.seek(REWARDS_AT + i * REWARD_BYTES);
                write_reward(reward, w);
            }
        }
        RunDecision::CardSelect(select) => write_select(s, select, w),
        RunDecision::Bundles(bundles) => {
            for (i, bundle) in bundles.iter().take(BUNDLES).enumerate() {
                w.seek(BUNDLES_AT + i * BUNDLE_CARDS * 3);
                for c in bundle.iter().take(BUNDLE_CARDS) {
                    card(w, c);
                }
            }
        }
        RunDecision::RestSite(options) => {
            w.seek(REST_AT);
            for option in options.iter().take(OPTIONS) {
                w.u8(option.kind as u8 + 1);
                w.u8(u8::from(option.enabled));
            }
        }
        RunDecision::Treasure(relic) => {
            w.seek(TREASURE_AT);
            w.u16(relic.map_or(vocab::PAD, vocab::relic_id));
        }
        RunDecision::Shop(shop) => {
            w.seek(SHOP_AT);
            for entry in shop.cards.iter().take(SHOP_CARDS) {
                match &entry.card {
                    Some(c) => {
                        card(w, c);
                        w.u8(u8::from(entry.on_sale));
                        w.i16(shop::price(s, entry.base_cost));
                    }
                    None => w.skip(6),
                }
            }
            w.seek(SHOP_AT + SHOP_CARDS * 6);
            for relic in shop.relics.iter().take(SHOP_RELICS) {
                match relic.id {
                    Some(id) => {
                        w.u16(vocab::relic_id(id));
                        w.i16(shop::price(s, relic.base_cost));
                    }
                    None => w.skip(4),
                }
            }
            w.seek(SHOP_AT + SHOP_CARDS * 6 + SHOP_RELICS * 4);
            for potion in shop.potions.iter().take(SHOP_POTIONS) {
                match potion.id {
                    Some(id) => {
                        w.u16(vocab::potion_id(id));
                        w.i16(shop::price(s, potion.base_cost));
                    }
                    None => w.skip(4),
                }
            }
            w.seek(SHOP_AT + SHOP_CARDS * 6 + (SHOP_RELICS + SHOP_POTIONS) * 4);
            w.i16(shop::price(s, shop.removal_base_cost));
            w.u8(u8::from(shop.removal_used));
        }
        RunDecision::Map(_)
        | RunDecision::Combat(_)
        | RunDecision::Unsupported(_)
        | RunDecision::Stopped(_)
        | RunDecision::GameOver
        | RunDecision::Victory => {}
    }
}

fn write_reward(reward: &Reward, w: &mut Writer<'_>) {
    match reward {
        Reward::Gold(amount) => {
            w.u8(1);
            w.u8(0);
            w.u16(*amount);
        }
        Reward::Potion(id) => {
            w.u8(2);
            w.u8(0);
            w.u16(vocab::potion_id(id));
        }
        Reward::Relic(id) | Reward::WaxRelic(id) => {
            w.u8(if matches!(reward, Reward::Relic(_)) {
                3
            } else {
                4
            });
            w.u8(0);
            w.u16(vocab::relic_id(id));
        }
        Reward::Card(cards) => {
            w.u8(5);
            w.u8(u8::from(cards.can_reroll));
            w.skip(4);
            for c in cards.cards.iter().take(REWARD_CARDS) {
                card(w, c);
            }
        }
        Reward::SpecialCard(c) => {
            w.u8(6);
            w.u8(0);
            card(w, c);
        }
    }
}

fn write_select(s: &RunState, select: &CardSelect, w: &mut Writer<'_>) {
    w.seek(SELECT_AT);
    w.u8(select_code(select.purpose));
    w.u8(select.min());
    w.u8(select.count);
    w.u8(select.picked.len());
    w.u8(u8::from(select.purpose.is_cancelable()));
    w.seek(SELECT_AT + 8);
    let offered = select.purpose.picks_offered();
    for (i, &option) in select.options.iter().take(SELECT_OPTIONS).enumerate() {
        let source = if offered { &select.offered } else { &s.deck };
        let Some(c) = source.get(option) else {
            w.skip(SELECT_OPTION_BYTES);
            continue;
        };
        card(w, c);
        w.u8(if offered {
            u8::MAX
        } else {
            u8::try_from(option).unwrap_or(u8::MAX)
        });
        w.u8(u8::from(select.picked.contains(&i)) | u8::from(c.enchantment.is_some()) << 1);
    }
}
