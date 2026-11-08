//! The observation (`docs/specs/rl-2x-plan.md`, section 7.1): little-endian plain integers,
//! no floats, no one-hot; unused slots are zero.
//!
//! Content kinds are [`crate::vocab`] ids (`u16`, 0 for an empty slot, 1 for content the
//! vocabulary does not list yet), stable across builds.
//!
//! **Static part** ([`STATIC_BYTES`], written with the dynamic part in the env batch): character, ascension, act, room type,
//! encounter (`u16`), the deck as (card `u16`, upgrade `u8`) × [`DECK`], relics (`u16`) ×
//! [`RELICS`], the belt (`u16`) × [`POTIONS`].
//!
//! **Dynamic part** ([`OBS_BYTES`], written every step), at these offsets:
//! - [`HEADER`]: player HP, max HP, block, energy, max energy, stars (`i16` each), turn, round
//!   (`u16`), hand, draw, discard, exhaust and play pile sizes, orb slots (`u8` each), status
//!   (`u8`: 0 player turn, 1 decision, 2 won, 3 lost, 4 unsupported).
//! - [`CREATURES_AT`]: allies then enemies, [`CREATURES`] × [`CREATURE_BYTES`]: alive, side,
//!   slot, 0 (`u8` each), monster (`u16`, 0 for none), HP, max HP, block (`i16`), powers ×
//!   [`POWERS`] as (kind `u16`, amount `i16`).
//! - [`CARDS_AT`]: every combat card in `CardId` order (creation order, so the draw pile's order
//!   never leaks), [`CARDS`] × [`CARD_BYTES`]: kind (`u16`), upgrade, pile (0 none, then
//!   hand, draw, discard, exhaust, play), cost (`i8`: the modified cost in hand, else the base),
//!   flags (`u8`: exhaust on next play, single-turn retain, single-turn Sly, dupe, afflicted,
//!   enchanted from bit 0).
//! - [`HAND_AT`]: the hand as card ids (`u16`) × [`HAND`], in hand order.
//! - [`INTENTS_AT`]: the enemies' intents in `enemies` order, [`INTENTS`] × [`INTENT_BYTES`]:
//!   kind + 1 (`u8`), damage per hit (`i16`), hits, status cards (`u8`).
//! - [`CHOICE_AT`]: a pending card choice: option card ids (`u16`) × [`OPTIONS`], min, max,
//!   picked (`u8`), 0.
//!
//! Values out of a field's range saturate. The modified costs are read through the combat's
//! listener list ([`Combat::energy_cost`]); the intents come from a cache ([`crate::intents`]).

use sts2_core::combat::{CombatState, DecisionRequest, Hooked, PileType};
use sts2_core::{Combat, Status};

use crate::action::{HAND_SLOTS, OPTION_SLOTS, POTION_SLOTS};
use crate::intents::IntentCache;
use crate::vocab;

/// Creature slots: allies first, then enemies.
pub const CREATURES: usize = 8;
/// Power slots per creature.
pub const POWERS: usize = 12;
/// Combat card slots.
pub const CARDS: usize = 64;
/// Hand slots.
pub const HAND: usize = HAND_SLOTS;
/// Intent slots, over all enemies.
pub const INTENTS: usize = 8;
/// Pending choice option slots.
pub const OPTIONS: usize = OPTION_SLOTS;
/// Deck slots of the static part.
pub const DECK: usize = 64;
/// Relic slots of the static part.
pub const RELICS: usize = 32;
/// Belt slots of the static part.
pub const POTIONS: usize = POTION_SLOTS;

/// Bytes of the header.
pub const HEADER: usize = 24;
/// Bytes per creature.
pub const CREATURE_BYTES: usize = 12 + POWERS * 4;
/// Bytes per card.
pub const CARD_BYTES: usize = 6;
/// Bytes per intent.
pub const INTENT_BYTES: usize = 5;
/// Offset of the creatures.
pub const CREATURES_AT: usize = HEADER;
/// Offset of the cards.
pub const CARDS_AT: usize = CREATURES_AT + CREATURES * CREATURE_BYTES;
/// Offset of the hand.
pub const HAND_AT: usize = CARDS_AT + CARDS * CARD_BYTES;
/// Offset of the intents.
pub const INTENTS_AT: usize = HAND_AT + HAND * 2;
/// Offset of the pending choice.
pub const CHOICE_AT: usize = INTENTS_AT + INTENTS * INTENT_BYTES;
/// Bytes of the dynamic part.
pub const OBS_BYTES: usize = CHOICE_AT + OPTIONS * 2 + 4;
/// Bytes of the static part.
pub const STATIC_BYTES: usize = 6 + DECK * 3 + RELICS * 2 + POTIONS * 2;

/// Little-endian fields into a fixed buffer, from a cursor.
pub(crate) struct Writer<'a> {
    buf: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    /// A writer over `buf`, zeroed.
    pub(crate) fn new(buf: &'a mut [u8]) -> Self {
        buf.fill(0);
        Self { buf, at: 0 }
    }

    pub(crate) const fn seek(&mut self, at: usize) {
        self.at = at;
    }

    /// Leaves `n` bytes zero.
    pub(crate) const fn skip(&mut self, n: usize) {
        self.at += n;
    }

    fn bytes<const N: usize>(&mut self, b: [u8; N]) {
        self.buf[self.at..self.at + N].copy_from_slice(&b);
        self.at += N;
    }

    pub(crate) fn u8(&mut self, v: impl TryInto<u8>) {
        self.bytes([v.try_into().unwrap_or(u8::MAX)]);
    }

    pub(crate) fn i8(&mut self, v: i32) {
        let v = i8::try_from(v).unwrap_or(if v < 0 { i8::MIN } else { i8::MAX });
        self.bytes(v.to_le_bytes());
    }

    pub(crate) fn u16(&mut self, v: impl TryInto<u16>) {
        self.bytes(v.try_into().unwrap_or(u16::MAX).to_le_bytes());
    }

    pub(crate) fn i16(&mut self, v: i32) {
        let v = i16::try_from(v).unwrap_or(if v < 0 { i16::MIN } else { i16::MAX });
        self.bytes(v.to_le_bytes());
    }
}

/// The pile code: 0 none, then [`PileType::LISTENER_ORDER`] from 1.
const fn pile_code(pile: Option<PileType>) -> u8 {
    match pile {
        None => 0,
        Some(PileType::Hand) => 1,
        Some(PileType::Draw) => 2,
        Some(PileType::Discard) => 3,
        Some(PileType::Exhaust) => 4,
        Some(PileType::Play) => 5,
    }
}

/// Writes the static part of `combat`'s observation into `out` ([`STATIC_BYTES`]).
///
/// # Panics
/// If `out` is shorter than [`STATIC_BYTES`].
pub fn observe_static(combat: &Combat, out: &mut [u8]) {
    let s = combat.state();
    let p = &s.player;
    let mut w = Writer::new(&mut out[..STATIC_BYTES]);
    w.u8(p.character as u8);
    w.u8(s.ascension);
    w.u8(s.act_index);
    w.u8(s.room_type as u8);
    w.u16(vocab::encounter(s.encounter));
    for card in s.deck.iter().take(DECK) {
        w.u16(vocab::card(card.kind));
        w.u8(card.upgrade_level);
    }
    w.seek(6 + DECK * 3);
    for relic in p.relics.iter().take(RELICS) {
        w.u16(vocab::relic(relic.kind));
    }
    w.seek(6 + DECK * 3 + RELICS * 2);
    for potion in p.potions.iter().take(POTIONS) {
        w.u16(potion.map_or(vocab::PAD, |p| vocab::potion(p.kind)));
    }
}

/// One env's observation writer and its intent cache.
#[derive(Debug)]
pub struct Observer {
    intents: bool,
    intent_cache: IntentCache,
}

impl Observer {
    /// An observer; without `intents` the intents section stays zero.
    #[must_use]
    pub fn new(intents: bool) -> Self {
        Self {
            intents,
            intent_cache: IntentCache::default(),
        }
    }

    /// Starts a new episode: no cached value of the last one is reused unless its whole key
    /// matches again.
    pub fn reset(&mut self) {
        self.intent_cache.reset();
    }

    /// Writes the dynamic part of `combat`'s observation into `out` ([`OBS_BYTES`]).
    ///
    /// # Panics
    /// If `out` is shorter than [`OBS_BYTES`].
    pub fn observe(&mut self, combat: &Combat, out: &mut [u8]) {
        let h = combat.hooked();
        let s = h.s;
        let mut w = Writer::new(&mut out[..OBS_BYTES]);
        header(s, combat.status(), &mut w);
        for (i, &id) in s
            .allies
            .iter()
            .chain(&s.enemies)
            .take(CREATURES)
            .enumerate()
        {
            let slot = if i < s.allies.len() {
                i
            } else {
                i - s.allies.len()
            };
            w.seek(CREATURES_AT + i * CREATURE_BYTES);
            let c = s.creature(id);
            w.u8(c.is_alive());
            w.u8(c.side as u8);
            w.u8(slot);
            w.u8(0);
            w.u16(c.monster.as_ref().map_or(0, |m| vocab::monster(&m.kind)));
            w.i16(c.hp);
            w.i16(c.max_hp);
            w.i16(c.block);
            for power in c.powers.iter().take(POWERS) {
                w.u16(vocab::power(power.kind));
                w.i16(power.amount);
            }
        }
        w.seek(CARDS_AT);
        for card in s.cards.iter().take(CARDS) {
            let pile = s.pile_of(card.id);
            w.u16(vocab::card(card.kind));
            w.u8(card.upgrade_level);
            w.u8(pile_code(pile));
            // The modified cost where it is payable now; elsewhere the base cost.
            w.i8(if pile == Some(PileType::Hand) {
                h.energy_cost(card.id)
            } else {
                card.energy_cost.base
            });
            w.u8(u8::from(card.exhaust_on_next_play)
                | u8::from(card.single_turn_retain) << 1
                | u8::from(card.single_turn_sly) << 2
                | u8::from(card.is_dupe) << 3
                | u8::from(card.affliction.is_some()) << 4
                | u8::from(card.enchantment.is_some()) << 5);
        }
        w.seek(HAND_AT);
        for card in s.player.hand.iter().take(HAND) {
            w.u16(card.0);
        }
        w.seek(INTENTS_AT);
        if self.intents {
            self.write_intents(h, &mut w);
        }
        w.seek(CHOICE_AT);
        choice(combat.pending_decision(), &mut w);
    }

    /// The intents section.
    fn write_intents(&mut self, h: Hooked<'_>, w: &mut Writer<'_>) {
        self.intent_cache.begin();
        let mut slots = INTENTS;
        for &enemy in &h.s.enemies {
            for intent in self.intent_cache.intents(h, enemy) {
                if slots == 0 {
                    return;
                }
                slots -= 1;
                w.u8(intent.kind as u8 + 1);
                w.i16(intent.attack.map_or(0, |a| a.damage));
                w.u8(intent.attack.map_or(0, |a| a.hits));
                w.u8(intent.cards.unwrap_or(0));
            }
        }
    }
}

/// The header section.
fn header(s: &CombatState, status: Status, w: &mut Writer<'_>) {
    let p = &s.player;
    let me = s.creature(p.creature);
    w.i16(me.hp);
    w.i16(me.max_hp);
    w.i16(me.block);
    w.i16(p.energy);
    w.i16(p.max_energy);
    w.i16(p.stars);
    w.u16(p.turn_number);
    w.u16(s.round);
    for pile in [&p.hand, &p.draw, &p.discard, &p.exhaust, &p.play] {
        w.u8(pile.len());
    }
    w.u8(p.orb_slots);
    w.u8(match status {
        Status::PlayerTurn => 0,
        Status::Decision => 1,
        Status::Won => 2,
        Status::Lost => 3,
        Status::Unsupported(_) => 4,
    });
}

/// The pending choice section.
fn choice(pending: Option<&DecisionRequest>, w: &mut Writer<'_>) {
    if let Some(DecisionRequest::ChooseCards {
        options,
        min,
        max,
        picked,
        ..
    }) = pending
    {
        for card in options.iter().take(OPTIONS) {
            w.u16(card.0);
        }
        w.seek(CHOICE_AT + OPTIONS * 2);
        w.u8(*min);
        w.u8(*max);
        w.u8(picked.len());
    }
}
