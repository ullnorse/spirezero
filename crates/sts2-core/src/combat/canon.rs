//! The solver's order-independent state hash (`docs/specs/solver.md`, section 2).
//!
//! Two states with the same hash play out the same: the hash covers the game state with the
//! labels left out (card, power, potion and orb ids, the id counters), the RNG streams left out
//! (the solver scripts them), and the hand, draw, discard and exhaust piles as multisets.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};

use super::CardPlay;
use super::history::{CombatHistory, HistoryEntry, HistoryKind};
use super::state::{Card, CardId, CombatState, Power};

/// Two independent 64-bit multiply-xorshift lanes.
///
/// Small values written through [`Hasher`] (`u8` to `u32`, as derived `Hash` writes struct
/// fields) are packed into whole words before mixing; a value that does not fit the word being
/// filled starts the next one, so the words depend only on the values and their widths.
#[derive(Clone, Copy, Debug)]
pub struct Hash128 {
    a: u64,
    b: u64,
    /// Packed bytes not yet mixed.
    buf: u64,
    /// Bytes in `buf`.
    len: u64,
}

impl Default for Hash128 {
    fn default() -> Self {
        Self::new(0)
    }
}

/// The splitmix64 finaliser.
const fn avalanche(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Hash128 {
    /// A hasher salted with `salt`.
    #[must_use]
    pub const fn new(salt: u64) -> Self {
        Self {
            a: 0x243F_6A88_85A3_08D3 ^ salt,
            b: 0x1319_8A2E_0370_7344 ^ salt.rotate_left(32),
            buf: 0,
            len: 0,
        }
    }

    /// Mixes one word into both lanes.
    #[inline]
    const fn mix(&mut self, x: u64) {
        self.a = (self.a.rotate_left(5) ^ x).wrapping_mul(0x517C_C1B7_2722_0A95);
        self.b = (self.b ^ x)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .rotate_left(29);
    }

    /// Mixes the packed bytes, if any, as one word.
    #[inline]
    const fn flush(&mut self) {
        if self.len > 0 {
            self.mix(self.buf ^ self.len << 59);
            self.buf = 0;
            self.len = 0;
        }
    }

    /// Packs a value of `bytes` bytes (below 8; `x < 2^(8 * bytes)`).
    #[inline]
    const fn pack_bytes(&mut self, x: u64, bytes: u64) {
        if self.len + bytes > 7 {
            self.flush();
        }
        self.buf |= x << (8 * self.len);
        self.len += bytes;
    }

    /// Mixes one word into both lanes (after the packed bytes before it).
    #[inline]
    pub const fn word(&mut self, x: u64) {
        self.flush();
        self.mix(x);
    }

    /// The 128-bit result.
    #[must_use]
    pub fn finish128(&self) -> u128 {
        let mut h = *self;
        h.flush();
        (u128::from(avalanche(h.a)) << 64) | u128::from(avalanche(h.b ^ 0x5851_F42D_4C95_7F2D))
    }

    /// Mixes a 128-bit value in as two words.
    #[inline]
    pub fn word128(&mut self, x: u128) {
        let (lo, hi) = halves(x);
        self.word(lo);
        self.word(hi);
    }
}

impl Hasher for Hash128 {
    fn finish(&self) -> u64 {
        let mut h = *self;
        h.flush();
        avalanche(h.a)
    }

    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for c in chunks {
            self.word(u64::from_le_bytes(*c));
        }
        if !rest.is_empty() {
            let mut w = [0; 8];
            w[..rest.len()].copy_from_slice(rest);
            self.word(u64::from_le_bytes(w) ^ (u64::try_from(rest.len()).unwrap_or(0) << 56));
        }
    }

    fn write_u8(&mut self, i: u8) {
        self.pack_bytes(u64::from(i), 1);
    }

    fn write_u16(&mut self, i: u16) {
        self.pack_bytes(u64::from(i), 2);
    }

    fn write_u32(&mut self, i: u32) {
        self.pack_bytes(u64::from(i), 4);
    }

    fn write_u64(&mut self, i: u64) {
        self.word(i);
    }

    fn write_usize(&mut self, i: usize) {
        self.word(u64::try_from(i).unwrap_or(u64::MAX));
    }

    fn write_i8(&mut self, i: i8) {
        self.write_u8(i.cast_unsigned());
    }

    fn write_i16(&mut self, i: i16) {
        self.write_u16(i.cast_unsigned());
    }

    fn write_i32(&mut self, i: i32) {
        self.write_u32(i.cast_unsigned());
    }

    fn write_i64(&mut self, i: i64) {
        self.word(i.cast_unsigned());
    }

    fn write_isize(&mut self, i: isize) {
        self.word(u64::try_from(i.cast_unsigned()).unwrap_or(u64::MAX));
    }
}

thread_local! {
    /// The last content hashed for each card slot ([`CombatState::card_hash`]), with its hash.
    static CARD_CACHE: RefCell<Vec<Option<(Card, u128)>>> = const { RefCell::new(Vec::new()) };
    /// [`CombatState::canonical_hash`]'s per-card hashes, kept for their buffer.
    static CARD_HASHES: RefCell<Vec<u128>> = const { RefCell::new(Vec::new()) };
}

/// A card without its labels (id, pile, deck slot, the card it was cloned from).
fn normalized(card: &Card) -> Card {
    let mut c = *card;
    c.id = CardId(0);
    c.pile = None;
    c.deck_slot = None;
    c.clone_of = c.clone_of.map(|_| CardId(0));
    c
}

/// `card`'s content hash (no cost lists) through slot `i` of the cache: reused while the
/// slot's normalised content stays the same.
fn cached_card_hash(cache: &mut Vec<Option<(Card, u128)>>, i: usize, card: &Card) -> u128 {
    if cache.len() <= i {
        cache.resize(i + 1, None);
    }
    let c = normalized(card);
    match &cache[i] {
        Some((cached, hash)) if *cached == c => *hash,
        _ => {
            let mut h = Hash128::new(0xCA4D);
            c.hash(&mut h);
            let hash = h.finish128();
            cache[i] = Some((c, hash));
            hash
        }
    }
}

/// The low and high 64 bits.
fn halves(x: u128) -> (u64, u64) {
    let b = x.to_le_bytes();
    let (lo, hi) = b.split_at(8);
    let word = |s: &[u8]| u64::from_le_bytes(s.try_into().unwrap_or([0; 8]));
    (word(lo), word(hi))
}

/// Sum of a multiset's element hashes (each already a finalised [`Hash128`]); the caller mixes
/// it in at its own position, which tells the multisets apart.
fn multiset(items: impl Iterator<Item = u128>) -> u128 {
    items.fold(0, u128::wrapping_add)
}

/// Packs up to four small values (each below 2^16) into one word.
fn pack(values: [u32; 4]) -> u64 {
    values.iter().fold(0, |w, &v| {
        (w << 16) ^ u64::from(v & 0xFFFF) ^ (u64::from(v >> 16) << 48)
    })
}

impl CombatState {
    /// A card's content, without its labels (id, pile, deck slot, the card it was cloned
    /// from), plus its cost lists: equal for cards that act the same.
    #[must_use]
    pub fn card_hash(&self, id: CardId) -> u128 {
        let mut h = Hash128::new(0xCA4D);
        normalized(&self.cards[id.index()]).hash(&mut h);
        if !self.cost_lists.is_empty() {
            self.cost_lists.energy(id).hash(&mut h);
            self.cost_lists.star(id).hash(&mut h);
        }
        h.finish128()
    }

    /// Every card's [`CombatState::card_hash`], by card index, into `out`. Most cards keep
    /// their content from state to state: the hash last computed for a slot is reused while
    /// its content stays the same.
    pub fn card_hashes(&self, out: &mut Vec<u128>) {
        out.clear();
        if !self.cost_lists.is_empty() {
            out.extend(
                (0..self.cards.len())
                    .map(|i| self.card_hash(CardId(u32::try_from(i).unwrap_or(u32::MAX)))),
            );
            return;
        }
        CARD_CACHE.with_borrow_mut(|cache| {
            out.extend(
                self.cards
                    .iter()
                    .enumerate()
                    .map(|(i, card)| cached_card_hash(cache, i, card)),
            );
        });
    }

    /// [`CombatState::card_hash`] in 64 bits: the key the draw chance events group cards by.
    #[must_use]
    pub fn card_key(&self, id: CardId) -> u64 {
        let hash = if self.cost_lists.is_empty() {
            CARD_CACHE.with_borrow_mut(|cache| {
                cached_card_hash(cache, id.index(), &self.cards[id.index()])
            })
        } else {
            self.card_hash(id)
        };
        halves(hash).0
    }

    /// Whether the solver's chance script answers this fight's draws (the `Shuffle` stream is
    /// scripted; see [`crate::rng::chance`]).
    #[must_use]
    #[inline]
    pub fn chance_scripted(&self) -> bool {
        self.rng[crate::rng::RunRngType::Shuffle].is_scripted()
    }

    /// Hands every stream of the fight (the run's and each monster's) to the chance script, or
    /// back to xoshiro.
    pub fn set_chance_scripted(&mut self, scripted: bool) {
        for t in crate::rng::RunRngType::ALL {
            self.rng[t].set_scripted(scripted);
        }
        for c in &mut self.creatures {
            if let Some(m) = &mut c.monster {
                m.rng.set_scripted(scripted);
            }
        }
    }

    /// The solver's state hash (`docs/specs/solver.md`, section 2).
    #[must_use]
    pub fn canonical_hash(&self) -> u128 {
        let mut h = Hash128::new(0x5017);
        let p = &self.player;
        h.word(pack([
            self.round,
            u32::from(self.in_progress)
                | u32::from(self.starting) << 1
                | u32::from(self.pending_loss) << 2,
            p.turn_number,
            p.effect_depth,
        ]));
        h.word(pack([
            p.energy.cast_unsigned(),
            p.max_energy.cast_unsigned(),
            p.stars.cast_unsigned(),
            u32::from(p.orb_slots)
                | u32::from(p.extra_turn) << 8
                | u32::from(p.ready_to_end_turn) << 9,
        ]));
        h.word(u64::from(p.gold.cast_unsigned()));
        self.side.hash(&mut h);
        p.phase.hash(&mut h);
        self.result.hash(&mut h);
        self.encounter_data.hash(&mut h);
        self.gold_proportion.hash(&mut h);
        for c in &self.creatures {
            h.word(pack([
                c.id.0,
                c.hp.cast_unsigned(),
                c.max_hp.cast_unsigned(),
                c.block.cast_unsigned(),
            ]));
            c.side.hash(&mut h);
            c.pet_owner.hash(&mut h);
            c.slot.hash(&mut h);
            c.infinite_hp_display.hash(&mut h);
            for power in &c.powers {
                self.hash_power(power, &mut h);
            }
            h.word(u64::from(u32::try_from(c.powers.len()).unwrap_or(u32::MAX)));
            if let Some(m) = &c.monster {
                m.kind.hash(&mut h);
                m.machine.hash(&mut h);
                h.word(pack([
                    u32::from(m.spawned_this_turn),
                    u32::from(m.is_performing_move),
                    u32::from(m.rolled),
                    1,
                ]));
            }
        }
        for (owner, power) in &self.removed_powers {
            owner.hash(&mut h);
            self.hash_power(power, &mut h);
        }
        self.allies.hash(&mut h);
        self.enemies.hash(&mut h);
        self.escaped.hash(&mut h);
        self.spawned_enemies.hash(&mut h);
        self.extra_rewards.hash(&mut h);
        self.removed_from_deck.hash(&mut h);
        for r in &p.relics {
            r.kind.hash(&mut h);
            r.melted.hash(&mut h);
            self.relic_data.get(r.id).hash(&mut h);
        }
        for slot in &p.potions {
            slot.map(|potion| potion.kind).hash(&mut h);
        }
        p.unported_potions.hash(&mut h);
        p.pets.hash(&mut h);
        for orb in &p.orbs {
            orb.kind.hash(&mut h);
            orb.value.hash(&mut h);
        }
        // Each card's content hash once: the piles and the history read them again and again.
        let mut cards = CARD_HASHES.take();
        self.card_hashes(&mut cards);
        let key = |c: &CardId| cards[c.index()];
        for pile in [&p.hand, &p.draw, &p.discard, &p.exhaust] {
            let sum = multiset(pile.iter().map(key));
            h.word128(sum);
            h.write_usize(pile.len());
        }
        for c in &p.play {
            let k = key(c);
            h.word128(k);
        }
        self.history.hash_canonical(&mut h, |c| cards[c.index()]);
        CARD_HASHES.set(cards);
        h.finish128()
    }

    /// A power without its id, with its instance data.
    fn hash_power(&self, power: &Power, h: &mut Hash128) {
        power.kind.hash(h);
        h.word(pack([
            power.amount.cast_unsigned(),
            power.amount_on_turn_start.cast_unsigned(),
            power.applier.map_or(u32::MAX, |c| c.0),
            u32::from(power.skip_next_duration_tick),
        ]));
        self.power_data.get(power.id).hash(h);
    }
}

impl CombatHistory {
    /// The history for [`CombatState::canonical_hash`]: counters as stored, this turn's entries
    /// as a multiset with card ids replaced by `card` and the play counter left out.
    pub(crate) fn hash_canonical(&self, h: &mut Hash128, card: impl Fn(CardId) -> u128) {
        let (turn, entries, turn_counts, counts, ethereal, hp_loss_hits, orbs, generated, last) =
            self.parts();
        turn.hash(h);
        turn_counts.hash(h);
        counts.hash(h);
        h.word(u64::from(ethereal) | u64::from(generated) << 32);
        hp_loss_hits.hash(h);
        orbs.hash(h);
        for a in last {
            a.map(|(turn, c)| (turn, card(c))).hash(h);
        }
        let sum = multiset(entries.iter().map(|e| {
            let mut eh = Hash128::new(0x1E);
            hash_entry(e, &mut eh, &card);
            eh.finish128()
        }));
        h.word128(sum);
    }
}

/// A play without its counter, its card as content.
fn hash_play(play: &CardPlay, h: &mut Hash128, card: &impl Fn(CardId) -> u128) {
    card(play.card).hash(h);
    play.target.hash(h);
    play.result_pile.hash(h);
    play.is_auto_play.hash(h);
    h.word(pack([
        play.play_index,
        play.play_count,
        play.energy_value.cast_unsigned(),
        play.star_value.cast_unsigned(),
    ]));
}

/// One history entry with card ids replaced by `card`.
fn hash_entry(e: &HistoryEntry, h: &mut Hash128, card: &impl Fn(CardId) -> u128) {
    e.actor.hash(h);
    e.round.hash(h);
    e.side.hash(h);
    e.turn_number.hash(h);
    e.kind.tag().hash(h);
    match e.kind {
        HistoryKind::CardPlayStarted(play) => hash_play(&play, h, card),
        HistoryKind::CardPlayFinished {
            play,
            card_type,
            was_ethereal,
        } => {
            hash_play(&play, h, card);
            card_type.hash(h);
            was_ethereal.hash(h);
        }
        HistoryKind::CardDrawn {
            card: c,
            from_hand_draw,
        } => {
            card(c).hash(h);
            from_hand_draw.hash(h);
        }
        HistoryKind::CardExhausted(c) | HistoryKind::CardDiscarded(c) => card(c).hash(h),
        HistoryKind::CardAfflicted {
            card: c,
            affliction,
        } => {
            card(c).hash(h);
            affliction.hash(h);
        }
        HistoryKind::CardGenerated { card: c, creator } => {
            card(c).hash(h);
            creator.hash(h);
        }
        HistoryKind::EnergySpent(n) | HistoryKind::StarsModified(n) => n.hash(h),
        HistoryKind::BlockGained {
            amount,
            props,
            play,
        } => {
            amount.hash(h);
            props.hash(h);
            if let Some(play) = play {
                hash_play(&play, h, card);
            }
        }
        HistoryKind::CreatureAttacked => {}
        HistoryKind::DamageReceived {
            result,
            props,
            dealer,
            card_source,
        } => {
            result.hash(h);
            props.hash(h);
            dealer.hash(h);
            card_source.map(card).hash(h);
        }
        HistoryKind::OrbChanneled(kind) => kind.hash(h),
        HistoryKind::PowerReceived {
            kind,
            amount,
            applier,
        } => {
            kind.hash(h);
            amount.hash(h);
            applier.hash(h);
        }
    }
}
