//! Action classes (`docs/plans/necro-e2e.md`, §1 "Action dedupe", WP1): legal actions that lead
//! to the same outcome, as the player sees it, merge into one class.
//!
//! | action | merges with another that has the same |
//! |---|---|
//! | play a card | canonical card ([`same_card`]) and target |
//! | use a potion in a fight | potion kind and target |
//! | pick a card of a combat card choice | canonical card |
//! | use a potion outside a fight | potion id |
//! | pick a card of a deck selection | card (id, upgrade, enchantment, counter) and picked flag |
//!
//! Every other action is a class of its own. A card's canonical form is everything about the
//! instance that can change what playing it does: its model, upgrades, keywords, costs (the
//! base, the local modifiers and the modified cost now), one-turn flags, affliction,
//! enchantment, replay count, its model's own fields, and whether it is a dupe, a clone or a deck
//! card. Not its id, its position in the hand, or which deck card it came from. All of it is on
//! the card's face or in the visible state; the hidden state (pile order, RNG) is never read
//! (`tests/classes.rs` permutes the draw pile and checks that the classes don't change).
//!
//! [`IndexMask::dedupe_with`] keeps one index per class in the mask, the class's *fixed member*:
//! its first member in legal-list order that has a bit. Picking that index plays that member.
//! Each kept index gets the class's member count (members that have a bit, plus members the
//! space has no index for that the engine accepts).

use sts2_core::Combat;
use sts2_core::combat::{Action, CardId, CreatureId, Decision, DecisionRequest, PileType};
use sts2_core::content::potions::PotionKind;
use sts2_core::run::{RunAction, RunCard, RunDecision, RunState};

use crate::action::IndexMask;
use crate::obs::{CardRecord, RECORD_FIELDS};

/// What an action merges by; `None` from the key functions is a class of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassKey {
    /// A card play: the card (compared by [`same_card`]) and target.
    Play {
        card: CardId,
        target: Option<CreatureId>,
    },
    /// A potion used in a fight.
    Potion {
        kind: PotionKind,
        target: Option<CreatureId>,
    },
    /// A card of a combat card choice (compared by [`same_card`]).
    Pick(CardId),
    /// A card of a combat card choice, compared by [`same_card`] and the pile it is in
    /// ([`RunOptions::pile_key`](crate::RunOptions::pile_key)).
    PickIn {
        card: CardId,
        pile: Option<PileType>,
    },
    /// A potion used outside a fight.
    BeltPotion(&'static str),
    /// A card of a deck selection.
    Select { card: RunCard, picked: bool },
}

impl ClassKey {
    /// Whether `self` and `other` are the same class in `combat` (the fight both were listed
    /// in, if any).
    #[must_use]
    pub fn same(&self, other: &Self, combat: Option<&Combat>) -> bool {
        match (self, other) {
            (
                Self::Play { card, target },
                Self::Play {
                    card: card2,
                    target: target2,
                },
            ) => target == target2 && combat.is_some_and(|c| same_card(c, *card, *card2)),
            (Self::Pick(a), Self::Pick(b)) => combat.is_some_and(|c| same_card(c, *a, *b)),
            (Self::PickIn { card: a, pile: pa }, Self::PickIn { card: b, pile: pb }) => {
                pa == pb && combat.is_some_and(|c| same_card(c, *a, *b))
            }
            (a, b) => a == b,
        }
    }
}

/// Whether the combat cards `a` and `b` are the same canonical card: everything about the
/// instance that can change what playing or picking it does is equal (see the module docs).
#[must_use]
pub fn same_card(combat: &Combat, first: CardId, second: CardId) -> bool {
    if first == second {
        return true;
    }
    let state = combat.state();
    let (x, y) = (state.card(first), state.card(second));
    let lists = state.cost_lists();
    let hooked = combat.hooked();
    x.kind == y.kind
        && x.upgrade_level == y.upgrade_level
        && x.keywords == y.keywords
        && x.energy_cost == y.energy_cost
        && x.star_cost == y.star_cost
        && x.exhaust_on_next_play == y.exhaust_on_next_play
        && x.single_turn_retain == y.single_turn_retain
        && x.single_turn_sly == y.single_turn_sly
        && x.affliction == y.affliction
        && x.enchantment == y.enchantment
        && x.base_replay_count == y.base_replay_count
        && x.data == y.data
        && x.is_dupe == y.is_dupe
        && x.is_clone() == y.is_clone()
        && x.deck_version().is_some() == y.deck_version().is_some()
        && lists.energy(first) == lists.energy(second)
        && lists.star(first) == lists.star(second)
        && hooked.energy_cost(first) == hooked.energy_cost(second)
        && hooked.star_cost(first) == hooked.star_cost(second)
}

/// The class key of a combat action in `combat`.
#[must_use]
pub fn combat_key(combat: &Combat, action: &Action) -> Option<ClassKey> {
    combat_key_with(combat, action, false)
}

/// [`combat_key`], with the pile in a card choice's key if `pile_key`.
#[must_use]
pub fn combat_key_with(combat: &Combat, action: &Action, pile_key: bool) -> Option<ClassKey> {
    match action {
        Action::PlayCard { card, target } => Some(ClassKey::Play {
            card: *card,
            target: *target,
        }),
        Action::UsePotion { slot, target } => {
            let potion = combat.state().player.potions.get(*slot)?.as_ref()?;
            Some(ClassKey::Potion {
                kind: potion.kind,
                target: *target,
            })
        }
        Action::Resolve(Decision::Pick(card)) => matches!(
            combat.pending_decision(),
            Some(DecisionRequest::ChooseCards { .. })
        )
        .then(|| {
            if pile_key {
                ClassKey::PickIn {
                    card: *card,
                    pile: combat.state().pile_of(*card),
                }
            } else {
                ClassKey::Pick(*card)
            }
        }),
        Action::EndTurn | Action::Resolve(_) => None,
    }
}

/// The class key of a run action at `decision` (with run state `s`).
#[must_use]
pub fn run_key(s: &RunState, decision: &RunDecision<'_>, action: &RunAction) -> Option<ClassKey> {
    run_key_with(s, decision, action, false)
}

/// [`run_key`], with the pile in a combat card choice's key if `pile_key`.
#[must_use]
pub fn run_key_with(
    s: &RunState,
    decision: &RunDecision<'_>,
    action: &RunAction,
    pile_key: bool,
) -> Option<ClassKey> {
    match (decision, action) {
        (RunDecision::Combat(combat), RunAction::Combat(a)) => combat_key_with(combat, a, pile_key),
        (RunDecision::CardSelect(select), RunAction::SelectCard(i)) => {
            let option = *select.options.get(*i)?;
            let source = if select.purpose.picks_offered() {
                &select.offered
            } else {
                &s.deck
            };
            Some(ClassKey::Select {
                card: *source.get(option)?,
                picked: select.picked.contains(i),
            })
        }
        (_, RunAction::UsePotion(slot)) => s
            .potions
            .get(*slot)
            .copied()
            .flatten()
            .map(ClassKey::BeltPotion),
        _ => None,
    }
}

/// What [`IndexMask::dedupe_with`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dedupe {
    /// Indices cleared because an earlier member of their class kept its index.
    pub merged: u32,
    /// Classes none of whose members has an index (so the agent cannot pick them).
    pub unmapped_classes: u32,
    /// Merged members whose card differs from the class's first member in a field the
    /// observation shows, per field of [`RECORD_FIELDS`] ([`record_mismatch`]).
    pub mismatches: [u32; RECORD_FIELDS.len()],
}

/// The [`RECORD_FIELDS`] in which the observation shows two cards differently (a bitmask).
///
/// Both actions must be card plays or picks, else 0. Members of one class must show the same:
/// the agent sees only one of them.
#[must_use]
pub fn record_mismatch(combat: &Combat, first: &Action, second: &Action) -> u8 {
    let card = |action: &Action| match action {
        Action::PlayCard { card, .. } | Action::Resolve(Decision::Pick(card)) => Some(*card),
        _ => None,
    };
    match (card(first), card(second)) {
        (Some(one), Some(other)) => {
            let hooked = combat.hooked();
            CardRecord::of(hooked, one).differs(&CardRecord::of(hooked, other))
        }
        _ => 0,
    }
}

/// One class while deduping: its key, its kept index and member count.
#[derive(Clone, Copy, Debug)]
struct Class {
    key: ClassKey,
    kept: Option<usize>,
    members: u8,
    /// The list position of the first member.
    first: Option<usize>,
}

/// Scratch space of [`IndexMask::dedupe_with`], kept between calls so it doesn't allocate.
#[derive(Clone, Debug, Default)]
pub struct DedupeScratch {
    classes: Vec<Class>,
}

impl<const N: usize, const W: usize> IndexMask<N, W> {
    /// Merges the mask's indices into classes, after it was filled from `legal` (and had the
    /// refused indices cleared): `index` is the fill's index function, `key` gives each action's
    /// [`ClassKey`] (`None`: a class of its own), `same` compares two keys, and `accepts` says
    /// whether the engine takes an action the space has no index for. Each class keeps its first
    /// member with a bit (in list order); `counts` (one per index) gets each kept index's member
    /// count, 0 for every other index.
    #[expect(
        clippy::too_many_arguments,
        reason = "the fill's functions, plus the outputs; a struct would only rename them"
    )]
    pub fn dedupe_with<T>(
        &mut self,
        legal: &[T],
        index: impl Fn(&T) -> Option<usize>,
        key: impl Fn(&T) -> Option<ClassKey>,
        same: impl Fn(&ClassKey, &ClassKey) -> bool,
        accepts: impl Fn(&T) -> bool,
        counts: &mut [u8],
        scratch: &mut DedupeScratch,
    ) -> Dedupe {
        self.dedupe_checked(legal, index, key, same, accepts, |_, _| 0, counts, scratch)
    }

    /// [`IndexMask::dedupe_with`], and `differs` (a bitmask over [`RECORD_FIELDS`]) compares each
    /// merged member with its class's first member; the fields that differ are counted in
    /// [`Dedupe::mismatches`].
    #[expect(
        clippy::too_many_arguments,
        reason = "dedupe_with's, plus the member check"
    )]
    pub fn dedupe_checked<T>(
        &mut self,
        legal: &[T],
        index: impl Fn(&T) -> Option<usize>,
        key: impl Fn(&T) -> Option<ClassKey>,
        same: impl Fn(&ClassKey, &ClassKey) -> bool,
        accepts: impl Fn(&T) -> bool,
        differs: impl Fn(&T, &T) -> u8,
        counts: &mut [u8],
        scratch: &mut DedupeScratch,
    ) -> Dedupe {
        counts.fill(0);
        for word in 0..W {
            let mut bits = self.bits()[word];
            while bits != 0 {
                counts[word * 64 + bits.trailing_zeros() as usize] = 1;
                bits &= bits - 1;
            }
        }
        let classes = &mut scratch.classes;
        classes.clear();
        let mut out = Dedupe::default();
        for (position, action) in legal.iter().enumerate() {
            let Some(k) = key(action) else {
                continue;
            };
            let slot = index(action);
            // A member with a bit: the first listed action of its index, not refused.
            let with_bit = slot.filter(|&i| self.position(i) == Some(position));
            // A member the space cannot express: no index, or an index an earlier listed action
            // took. (The first listed action of an index without a bit was refused.)
            let without_index = slot.is_none()
                || (with_bit.is_none() && legal[..position].iter().any(|a| index(a) == slot));
            let member = with_bit.is_some() || (without_index && accepts(action));
            match classes.iter_mut().find(|c| same(&c.key, &k)) {
                Some(class) => {
                    if !member {
                        continue;
                    }
                    match class.first {
                        Some(first) => {
                            let bits = differs(&legal[first], action);
                            for (k, n) in out.mismatches.iter_mut().enumerate() {
                                *n += u32::from(bits >> k & 1);
                            }
                        }
                        None => class.first = Some(position),
                    }
                    class.members = class.members.saturating_add(1);
                    match (class.kept, with_bit) {
                        (Some(_), Some(i)) => {
                            self.remove(i);
                            counts[i] = 0;
                            out.merged += 1;
                        }
                        (None, Some(i)) => class.kept = Some(i),
                        (_, None) => {}
                    }
                }
                None => classes.push(Class {
                    key: k,
                    kept: with_bit,
                    members: u8::from(member),
                    first: member.then_some(position),
                }),
            }
        }
        for class in classes.iter() {
            match class.kept {
                Some(i) => counts[i] = class.members,
                None if class.members > 0 => out.unmapped_classes += 1,
                None => {}
            }
        }
        out
    }
}
