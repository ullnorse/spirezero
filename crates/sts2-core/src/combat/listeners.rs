//! Hook presence as unions kept in the state, and the listener walk (stage 1 of
//! `docs/specs/rl-2x-plan.md`, section 2.3).
//!
//! The listeners are a function of the state (`CombatState.cs:411-470`): for each creature in
//! combat (allies, then enemies) its powers in application order, then its monster, or for the
//! player active for hooks its relics, its potions by slot and the cards of Hand, Draw, Discard,
//! Exhaust and Play, each followed by its affliction, then its enchantment. Instead of a cached
//! list of them, the state keeps the hooks they override:
//! - [`Creature::hooks`]: the union of the creature's powers' and monster's hooks;
//! - [`Player::item_hooks`]: the union of the unmelted relics' and the potions' hooks;
//! - [`HookIndex`]: for the card listeners (a card in a combat pile, its affliction, its
//!   enchantment) a count per hook, the hooks with a count, and the cards that override any
//!   hook; and the union of everything a walk reaches ([`CombatState::hooks`]).
//!
//! So whether a hook has a listener is one bit test ([`CombatState::lacks`]), and a pass walks
//! the state in listener order ([`CombatState::visit_listeners`]), skipping creatures, items and
//! cards whose union lacks the hook. The cards come from the listening cards, put in listener
//! order (pile, then place in the pile) at pass time, so a move between piles changes nothing
//! here; a card entering or leaving the piles, or gaining or losing an affliction or an
//! enchantment, does.
//!
//! The unions change in the [`CombatState`] methods that change what listens: the power
//! methods, [`CombatState::push_creature`], the ally and enemy lists, [`CombatState::set_potion`],
//! the pile methods, [`CombatState::set_affliction`], [`CombatState::set_enchantment`] and
//! [`CombatState::set_hooks_active`]. Relics, monsters and card kinds do not change in combat.
//! Debug and `regress` builds check every union test against a walk of every listener and every
//! pass against the old listener list, built from scratch ([`CombatState::listener_entries`]).
//!
//! [`Creature::hooks`]: super::state::Creature::hooks
//! [`Player::item_hooks`]: super::state::Player::item_hooks

use smallvec::SmallVec;

use super::hooks::{Bound, Hook, HookSet, PowerRef};
use super::state::{CardId, CombatState, CreatureId, PileType};

/// A pass's listeners, in order. Inline for a pass of up to eight.
pub type Listeners = SmallVec<[Bound; 8]>;

/// The state's part of the hook unions ([module docs](self)). Derived data, so it is left out
/// of equality and hashing, like [`PromptCount`](super::state::PromptCount): the order of the
/// listening cards depends on the fight's history, and a unit test's hand-made state need not
/// keep the index in step.
#[derive(Debug)]
pub struct HookIndex {
    /// The union of the hooks a walk reaches: every creature in combat's, and for the player
    /// active for hooks, its items' and its cards'.
    hooks: HookSet,
    /// The hooks with a nonzero count in `card_counts`.
    card_hooks: HookSet,
    /// The player's items' and cards' hooks while it is active for hooks, else none.
    player_hooks: HookSet,
    /// Per hook ([`Hook::index`]), how many card listeners in a combat pile override it.
    card_counts: [u16; Hook::COUNT],
    /// The cards in a combat pile whose model, affliction or enchantment overrides a hook, in
    /// no particular order.
    listening_cards: Vec<CardId>,
}

impl_clone_by_fields!(HookIndex {
    hooks,
    card_hooks,
    player_hooks,
    card_counts,
    listening_cards: copy,
});

impl Default for HookIndex {
    fn default() -> Self {
        Self {
            hooks: HookSet::EMPTY,
            card_hooks: HookSet::EMPTY,
            player_hooks: HookSet::EMPTY,
            card_counts: [0; Hook::COUNT],
            listening_cards: Vec::new(),
        }
    }
}

impl PartialEq for HookIndex {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for HookIndex {}

impl std::hash::Hash for HookIndex {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}

/// The piles in listener order are the piles in declaration order, so a card's pile is its
/// rank in the walk.
const _: () = {
    let mut i = 0;
    while i < PileType::LISTENER_ORDER.len() {
        assert!(PileType::LISTENER_ORDER[i].index() == i);
        i += 1;
    }
};

impl CombatState {
    /// The union of the hooks some listener overrides. Debug builds check it against a walk
    /// of every listener.
    #[inline]
    #[must_use]
    pub(crate) fn hooks(&self) -> HookSet {
        #[cfg(debug_assertions)]
        self.check_hooks();
        self.hook_index.hooks
    }

    /// Whether no listener overrides `hook`: a pass of it would visit nobody. Debug builds
    /// check the union against a walk of every listener.
    #[inline]
    #[must_use]
    pub(crate) fn lacks(&self, hook: Hook) -> bool {
        #[cfg(debug_assertions)]
        self.check_hooks();
        !self.hook_index.hooks.contains(hook)
    }

    /// Calls `visit` on each listener overriding `hook`, in listener order. A sync pass visits
    /// them directly (it cannot change the listeners); an async pass builds its snapshot.
    /// Inlined, so that in a pass of a known hook each hook test is one instruction. Debug
    /// builds compare the listeners with the old listener list.
    #[inline]
    pub(crate) fn visit_listeners(&self, hook: Hook, mut visit: impl FnMut(Bound)) {
        #[cfg(debug_assertions)]
        let mut seen = Vec::new();
        let mut visit = |bound: Bound| {
            #[cfg(debug_assertions)]
            seen.push(bound);
            visit(bound);
        };
        let bit = hook.word_mask();
        for &id in &self.allies {
            self.visit_creature(id, hook, bit, &mut visit);
        }
        for &id in &self.enemies {
            self.visit_creature(id, hook, bit, &mut visit);
        }
        #[cfg(debug_assertions)]
        self.check_pass(hook, None, &seen);
    }

    /// [`CombatState::visit_listeners`] for the creature `id` in combat: its powers, then its
    /// monster, or for the player its relics, potions and cards. `bit` is `hook`'s
    /// [`Hook::word_mask`].
    #[inline(always)]
    #[expect(
        clippy::inline_always,
        reason = "called twice per pass; out of line it pays a call and a frame per creature"
    )]
    fn visit_creature(
        &self,
        id: CreatureId,
        hook: Hook,
        bit: (usize, u64),
        visit: &mut impl FnMut(Bound),
    ) {
        let creature = self.creature(id);
        if creature.hooks.has_word(bit) {
            for p in &creature.powers {
                if p.kind.hooks_ref().has_word(bit) {
                    visit(Bound::Power(
                        p.kind,
                        PowerRef {
                            owner: id,
                            id: p.id,
                        },
                    ));
                }
            }
            if creature.monster_hooks.has_word(bit) {
                visit(Bound::Monster(id));
            }
        }
        // `player_hooks` is empty unless the player is active for hooks.
        if creature.monster.is_none() && self.hook_index.player_hooks.has_word(bit) {
            let mut items = Listeners::new();
            self.items_and_cards_into(hook, &mut items);
            for &bound in &items {
                visit(bound);
            }
        }
    }

    /// [`CombatState::visit_listeners`] for the player's relics, potions and cards. Out of line:
    /// its loads would otherwise be hoisted into every walk.
    #[inline(never)]
    fn items_and_cards_into(&self, hook: Hook, out: &mut Listeners) {
        let player = &self.player;
        if player.item_hooks.contains(hook) {
            for r in player.relics.iter().filter(|r| !r.melted) {
                if r.kind.hooks().contains(hook) {
                    out.push(Bound::Relic(r.kind, r.id));
                }
            }
            for p in player.potions() {
                if p.kind.hooks().contains(hook) {
                    out.push(Bound::Potion(p.kind, p.id));
                }
            }
        }
        if !self.hook_index.card_hooks.contains(hook) {
            return;
        }
        // The listening cards overriding `hook`, by pile (in listener order) and place in it.
        let mut cards: SmallVec<[(usize, usize, CardId); 4]> = SmallVec::new();
        for &card in &self.hook_index.listening_cards {
            if self.card_hooks(card).iter().any(|h| h.contains(hook)) {
                let pile = self.card(card).pile.expect("a listening card is in a pile");
                cards.push((pile.index(), 0, card));
            }
        }
        if cards.len() > 1 {
            for (pile, place, card) in &mut cards {
                *place = self
                    .player
                    .pile(PileType::LISTENER_ORDER[*pile])
                    .iter()
                    .position(|c| c == card)
                    .expect("a card is in the pile it names");
            }
            cards.sort_unstable();
        }
        for &(_, _, id) in &cards {
            let card = self.card(id);
            if card.kind.hooks().contains(hook) {
                out.push(Bound::Card(id));
            }
            if card
                .affliction
                .is_some_and(|a| a.kind.hooks().contains(hook))
            {
                out.push(Bound::Affliction(id));
            }
            if card
                .enchantment
                .is_some_and(|e| e.kind.hooks().contains(hook))
            {
                out.push(Bound::Enchantment(id));
            }
        }
    }

    /// The hooks of `card`'s listeners: its model, its affliction, its enchantment (empty for
    /// none).
    fn card_hooks(&self, card: CardId) -> [HookSet; 3] {
        let c = self.card(card);
        [
            c.kind.hooks(),
            c.affliction.map_or(HookSet::EMPTY, |a| a.kind.hooks()),
            c.enchantment.map_or(HookSet::EMPTY, |e| e.kind.hooks()),
        ]
    }

    /// The player's items' and cards' hooks with `card_hooks` as the cards', if the player is
    /// active for hooks, else none.
    fn player_union(&self, card_hooks: HookSet) -> HookSet {
        if self.player.hooks_active {
            self.player.item_hooks.union(card_hooks)
        } else {
            HookSet::EMPTY
        }
    }

    /// The union a walk reaches, with `player` as the player's items' and cards'.
    fn walk_union(&self, player: HookSet) -> HookSet {
        let mut hooks = HookSet::EMPTY;
        for &id in self.allies.iter().chain(&self.enemies) {
            let creature = &self.creatures[id.index()];
            hooks = hooks.union(creature.hooks);
            if creature.monster.is_none() {
                hooks = hooks.union(player);
            }
        }
        hooks
    }

    /// Recomputes the union a walk reaches: after a creature's or the items' union changed, or
    /// what is in combat, or `hooks_active`.
    pub(super) fn refresh_hooks(&mut self) {
        let player = self.player_union(self.hook_index.card_hooks);
        self.hook_index.player_hooks = player;
        self.hook_index.hooks = self.walk_union(player);
    }

    /// Counts the listeners of `card` into the card unions (`enter`: it just entered a combat
    /// pile, or it is in one and its affliction or enchantment just changed) or out of them
    /// (it is about to leave the piles, or its affliction or enchantment to change).
    pub(super) fn count_card(&mut self, card: CardId, enter: bool) {
        let sets = self.card_hooks(card);
        if self.hook_index.count(card, sets, enter) {
            self.refresh_hooks();
        }
    }

    /// Forgets every card listener: the piles were just emptied.
    pub(super) fn forget_card_listeners(&mut self) {
        self.hook_index = HookIndex::default();
        self.refresh_hooks();
    }

    /// The card unions, counted from the piles (`hooks` left empty).
    fn count_card_index(&self) -> HookIndex {
        let mut index = HookIndex::default();
        for pile in PileType::LISTENER_ORDER {
            for &card in self.player.pile(pile) {
                index.count(card, self.card_hooks(card), true);
            }
        }
        index
    }

    /// Recounts every hook union from scratch: once the state is built, and after a unit test
    /// changes it directly (`Combat::edit`).
    pub(crate) fn recount_hooks(&mut self) {
        for creature in &mut self.creatures {
            creature.monster_hooks = creature.count_monster_hooks();
            creature.hooks = creature.count_hooks();
        }
        self.player.item_hooks = self.player.count_item_hooks();
        self.hook_index = self.count_card_index();
        self.refresh_hooks();
    }

    /// Whether every hook union equals a recount, and the union a walk reaches equals the
    /// union of every listener's hooks ([`CombatState::listener_entries`]).
    #[cfg(any(test, debug_assertions))]
    pub(crate) fn hook_unions_are_exact(&self) -> bool {
        let fresh = self.count_card_index();
        let sorted = |cards: &[CardId]| {
            let mut cards = cards.to_vec();
            cards.sort_unstable();
            cards
        };
        let index = &self.hook_index;
        self.creatures
            .iter()
            .all(|c| c.monster_hooks == c.count_monster_hooks() && c.hooks == c.count_hooks())
            && self.player.item_hooks == self.player.count_item_hooks()
            && index.card_counts == fresh.card_counts
            && index.card_hooks == fresh.card_hooks
            && sorted(&index.listening_cards) == sorted(&fresh.listening_cards)
            && index.player_hooks == self.player_union(fresh.card_hooks)
            && index.hooks == self.walk_union(index.player_hooks)
            && index.hooks == self.listener_entries(&mut Vec::new())
    }
}

impl HookIndex {
    /// Counts the listeners of `card`, whose hooks are `sets`, in (`enter`) or out. Returns
    /// whether `card_hooks` changed.
    fn count(&mut self, card: CardId, sets: [HookSet; 3], enter: bool) -> bool {
        if sets.iter().all(|h| h.is_empty()) {
            return false;
        }
        let before = self.card_hooks;
        for set in sets {
            for hook in set.indices() {
                let count = &mut self.card_counts[hook];
                if enter {
                    *count = count
                        .checked_add(1)
                        .expect("fewer than 65,536 card listeners of one hook");
                } else {
                    *count -= 1;
                }
                self.card_hooks.set_index(hook, *count > 0);
            }
        }
        if enter {
            self.listening_cards.push(card);
        } else {
            let at = self
                .listening_cards
                .iter()
                .position(|&c| c == card)
                .expect("a listening card leaving the piles is listed");
            self.listening_cards.swap_remove(at);
        }
        self.card_hooks != before
    }
}

/// The old listener walk: the oracle the unions and passes are checked against.
#[cfg(any(test, debug_assertions))]
mod oracle {
    use super::super::hooks::{Hook, HookSet, Listener};
    use super::super::state::{CardId, CombatState, CreatureId, PileType};
    use super::Bound;

    /// A listener with the hooks its model overrides.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Entry {
        pub listener: Listener,
        pub hooks: HookSet,
    }

    impl CombatState {
        /// Combat listeners in dispatch order (`CombatState.cs:411-470`): for each ally (the
        /// player, then pets such as Osty in insertion order) then enemy (slot order), its
        /// powers in application order, then its monster model, or for a player active for
        /// hooks its relics, its potions by slot and the cards of Hand, Draw, Discard, Exhaust
        /// and Play, each followed by its affliction, then its enchantment. Run-level listeners
        /// put deck cards first; the engine has no deck-card listeners.
        ///
        /// Calls `push` on every listener, in order, with the hooks its model overrides.
        pub(crate) fn for_each_listener(&self, mut push: impl FnMut(Listener, HookSet)) {
            for &id in self.allies.iter().chain(&self.enemies) {
                self.for_each_listener_of(id, &mut push);
            }
        }

        /// The listeners of the creature `id` in combat, in
        /// [`CombatState::for_each_listener`]'s order.
        fn for_each_listener_of(&self, id: CreatureId, push: &mut impl FnMut(Listener, HookSet)) {
            let creature = self.creature(id);
            for p in &creature.powers {
                let listener = Listener::Power {
                    owner: id,
                    power: p.id,
                };
                push(listener, p.kind.hooks());
            }
            if let Some(monster) = &creature.monster {
                push(Listener::Monster(id), monster.kind.hooks());
            } else if self.player.hooks_active {
                // `CombatState.IterateHookListeners`: not the melted relics.
                for r in self.player.relics.iter().filter(|r| !r.melted) {
                    push(Listener::Relic(r.id), r.kind.hooks());
                }
                for p in self.player.potions() {
                    push(Listener::Potion(p.id), p.kind.hooks());
                }
                for pile in PileType::LISTENER_ORDER {
                    for &c in self.player.pile(pile) {
                        self.for_each_listener_of_card(c, push);
                    }
                }
            }
        }

        /// The listeners of the card `c` (in a pile of a player active for hooks): the card,
        /// then its affliction, then its enchantment.
        fn for_each_listener_of_card(&self, c: CardId, push: &mut impl FnMut(Listener, HookSet)) {
            let card = self.card(c);
            push(Listener::Card(c), card.kind.hooks());
            if let Some(affliction) = card.affliction {
                push(Listener::Affliction(c), affliction.kind.hooks());
            }
            if let Some(enchantment) = card.enchantment {
                push(Listener::Enchantment(c), enchantment.kind.hooks());
            }
        }

        /// Fills `out` (cleared first) with each listener that overrides a hook, with the hooks
        /// its model overrides, and returns the union of those hooks. A listener overriding
        /// none (a Strike) is left out: every reader filters by hook.
        pub(crate) fn listener_entries(&self, out: &mut Vec<Entry>) -> HookSet {
            out.clear();
            let mut union = HookSet::EMPTY;
            self.for_each_listener(|listener, hooks| {
                if !hooks.is_empty() {
                    union = union.union(hooks);
                    out.push(Entry { listener, hooks });
                }
            });
            union
        }

        /// The model of `listener`'s kind, as the old pass bound it at the call, for the ids
        /// whose kind a [`Bound`] carries.
        fn bound_matches(&self, bound: Bound) -> bool {
            match bound {
                Bound::Power(kind, me) => self.power(me.owner, me.id).kind == kind,
                Bound::Relic(kind, id) => self.player.relics[id.index()].kind == kind,
                Bound::Potion(kind, id) => {
                    self.player.potions().any(|p| p.id == id && p.kind == kind)
                }
                Bound::Monster(_)
                | Bound::Card(_)
                | Bound::Affliction(_)
                | Bound::Enchantment(_) => true,
            }
        }

        /// Asserts that the union a walk reaches is the union of every listener's hooks.
        pub(super) fn check_hooks(&self) {
            assert!(
                self.hook_index.hooks == self.listener_entries(&mut Vec::new()),
                "a hook union is out of step with the listeners"
            );
        }

        /// Asserts that `pass` is the old listener list filtered by `hook` and `only`, with the
        /// kinds the old pass would have bound, and that every union equals a recount.
        pub(super) fn check_pass(&self, hook: Hook, only: Option<&[Listener]>, pass: &[Bound]) {
            let mut entries = Vec::new();
            self.listener_entries(&mut entries);
            let old: Vec<Listener> = entries
                .iter()
                .filter(|e| e.hooks.contains(hook) && only.is_none_or(|o| o.contains(&e.listener)))
                .map(|e| e.listener)
                .collect();
            let new: Vec<Listener> = pass.iter().map(|b| b.listener()).collect();
            assert_eq!(
                new, old,
                "the walk of {hook:?} differs from the listener list"
            );
            assert!(
                pass.iter().all(|&b| self.bound_matches(b)),
                "the walk of {hook:?} bound a wrong kind"
            );
            assert!(self.hook_unions_are_exact(), "a hook union is out of step");
        }
    }
}

#[cfg(test)]
pub use oracle::Entry;

#[cfg(test)]
mod tests {
    use super::super::hooks::{Hook, Listener};
    use super::Listeners;
    use crate::combat::CombatState;
    use crate::combat::{DeckCard, Encounter};
    use crate::content::cards::{CardKind, pools};
    use crate::content::potions::PotionKind;
    use crate::content::relics::RelicKind;
    use crate::{Action, Character, Combat, CombatConfig, Status};

    /// `xorshift64*`: the test's own RNG (not game logic).
    struct Fuzz(u64);

    impl Fuzz {
        const fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn below(&mut self, n: usize) -> usize {
            usize::try_from(self.next() % u64::try_from(n).expect("fits u64")).expect("below n")
        }

        fn pick<T: Copy>(&mut self, items: &[T]) -> T {
            items[self.below(items.len())]
        }
    }

    /// Whether `id` is in another character's pool.
    fn other_characters(
        character: Character,
        id: &str,
        pool: fn(Character) -> Vec<&'static str>,
    ) -> bool {
        Character::ALL
            .into_iter()
            .filter(|&c| c != character)
            .any(|c| pool(c).contains(&id))
    }

    /// A random combat as `fuzz_combats` builds them: the starter deck plus random cards of
    /// the character's and the colorless pool (some upgraded), random relics and potions.
    fn config(
        fuzz: &mut Fuzz,
        i: usize,
        character: Character,
        encounter: Encounter,
    ) -> CombatConfig {
        let ascension = u8::try_from(fuzz.below(11)).expect("0..=10");
        let mut config = CombatConfig::new(character, &format!("UNION{i}"), ascension)
            .expect("every starter card is ported")
            .with_encounter(encounter);
        let cards: Vec<CardKind> = pools::character(character)
            .into_iter()
            .chain(pools::colorless())
            .filter_map(pools::PoolCard::kind)
            .collect();
        for _ in 0..fuzz.below(16) {
            let kind = fuzz.pick(&cards);
            let mut card = DeckCard::new(kind);
            if fuzz.below(3) == 0 && kind.is_upgradable(0) {
                card.upgrade_level = 1;
            }
            config.deck.push(card);
        }
        let relic_pool = |c: Character| c.relic_pool().iter().map(|&(id, _)| id).collect();
        let relics: Vec<RelicKind> = RelicKind::ALL
            .into_iter()
            .filter(|r| !other_characters(character, r.id(), relic_pool))
            .collect();
        for _ in 0..fuzz.below(6) {
            let kind = fuzz.pick(&relics);
            if config.relics.iter().all(|r| r.kind != kind) {
                config.relics.push(kind.into());
            }
        }
        let potion_pool = |c: Character| c.potion_pool().iter().map(|&(id, _)| id).collect();
        let potions: Vec<PotionKind> = PotionKind::ALL
            .into_iter()
            .filter(|p| !other_characters(character, p.id(), potion_pool))
            .collect();
        let belt: Vec<PotionKind> = (0..fuzz.below(config.potion_slots + 1))
            .map(|_| fuzz.pick(&potions))
            .collect();
        config.with_potions(&belt)
    }

    /// Asserts that every union equals the union of its listeners' `HOOKS`, and that the walk
    /// of every hook is the old listener list filtered by the hook.
    fn check(s: &CombatState, at: &str) {
        assert!(s.hook_unions_are_exact(), "{at}: a union is out of step");
        let mut entries = Vec::new();
        s.listener_entries(&mut entries);
        for hook in Hook::ALL {
            let mut walk = Listeners::new();
            s.visit_listeners(hook, |bound| walk.push(bound));
            let walked: Vec<Listener> = walk.iter().map(|b| b.listener()).collect();
            let listed: Vec<Listener> = entries
                .iter()
                .filter(|e| e.hooks.contains(hook))
                .map(|e| e.listener)
                .collect();
            assert_eq!(walked, listed, "{at}: the walk of {hook:?}");
            assert_eq!(
                s.lacks(hook),
                listed.is_empty(),
                "{at}: whether {hook:?} has listeners"
            );
        }
    }

    #[test]
    fn unions_are_the_or_of_the_listeners_hooks_on_random_combats() {
        let combats: usize = std::env::var("FUZZ_UNIONS")
            .ok()
            .map_or(60, |n| n.parse().expect("FUZZ_UNIONS is a number"));
        let encounters = Encounter::ALL;
        let mut fuzz = Fuzz(0x9E37_79B9_7F4A_7C15);
        for i in 0..combats {
            let character = Character::ALL[i % Character::ALL.len()];
            let encounter = encounters[fuzz.below(encounters.len())];
            let mut combat = Combat::new(config(&mut fuzz, i, character, encounter));
            for step in 0..400 {
                check(
                    combat.state(),
                    &format!("combat {i} ({character:?}, {encounter:?}), step {step}"),
                );
                if !matches!(combat.status(), Status::PlayerTurn | Status::Decision) {
                    break;
                }
                let mut actions = combat.legal_actions();
                let end_turn = actions.iter().position(|a| matches!(a, Action::EndTurn));
                if let Some(end) = end_turn
                    && actions.len() > 1
                    && fuzz.below(4) != 0
                {
                    actions.swap_remove(end);
                }
                let action = actions.swap_remove(fuzz.below(actions.len()));
                combat.apply(action).expect("a legal action is accepted");
            }
        }
    }
}
