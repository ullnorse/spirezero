//! Card pile commands (`CardPileCmd`, `CardCmd.Exhaust`): add, draw, shuffle, flush.

use crate::combat::engine::{Engine, Event, Flow};
use crate::combat::history::HistoryKind;
use crate::combat::state::{Card, CardId, CombatState, CreatureId, MovedCards, PileType};
use crate::content::cards::{self, CardKeyword, CardKind};
use crate::decimal::Decimal;
use crate::rng::RunRngType;
use crate::shuffle::stable_shuffle_by_key;
use smallvec::SmallVec;

/// A hand's worth of cards, inline.
pub type Cards = SmallVec<[CardId; 10]>;

/// `CardPile.MaxCardsInHand`.
pub const MAX_HAND_SIZE: usize = 10;

/// `CardPilePosition`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Bottom,
    Top,
    /// An index drawn on `Rng.Shuffle` for each card (`NextInt(count + 1)`).
    Random,
}

impl Engine<'_> {
    /// `CardPileCmd.Add(card, pile, position)`: [`Engine::add_cards_to_pile`] for one card,
    /// without the batch's buffer.
    pub(crate) fn add_to_pile(&mut self, card: CardId, pile: PileType, position: Position) -> Flow {
        if !self.can_add_to_pile() {
            return Ok(());
        }
        self.add_allowed_to_pile(card, pile, position)
    }

    /// [`Engine::add_to_pile`] once [`Engine::can_add_to_pile`] holds.
    fn add_allowed_to_pile(&mut self, card: CardId, pile: PileType, position: Position) -> Flow {
        let old = self.move_to_pile(card, pile, position)?;
        self.after_card_moved(card, old)
    }

    /// Whether `CardPileCmd.Add` adds anything: not while combat is ending, for a dead owner,
    /// or outside a live combat.
    #[inline]
    fn can_add_to_pile(&self) -> bool {
        self.s.in_progress && !self.player_dead() && !self.hooked().is_ending()
    }

    /// One card of [`Engine::add_cards_to_pile`]'s first loop: the move (to the discard pile
    /// instead of a full hand), then `AfterCardEnteredCombat` for a card that had no pile.
    /// Returns the old pile.
    fn move_to_pile(
        &mut self,
        card: CardId,
        pile: PileType,
        position: Position,
    ) -> Flow<Option<PileType>> {
        let target = if pile == PileType::Hand && self.s.player.hand.len() >= MAX_HAND_SIZE {
            PileType::Discard
        } else {
            pile
        };
        let old = self.s.move_card(card, target, |s| match position {
            Position::Bottom => None,
            Position::Top => Some(0),
            Position::Random => Some(random_pile_index(s, target)),
        });
        if old.is_none() {
            self.after_card_entered_combat(card)?;
        }
        Ok(old)
    }

    /// One card of [`Engine::add_cards_to_pile`]'s second loop: `AfterCardChangedPiles` if the
    /// card's pile type changed from `old`.
    fn after_card_moved(&mut self, card: CardId, old: Option<PileType>) -> Flow {
        if self.lacks_after_card_changed_piles() {
            return Ok(());
        }
        if old.is_none() || old != self.s.pile_of(card) {
            self.after_card_changed_piles(card, old)?;
        }
        Ok(())
    }

    /// `CardPileCmd.Add(cards, pile, position)` (`CardPileCmd.cs:373-545`), one batch: a no-op
    /// while combat is ending, for a dead owner, or outside a live combat. Each card moves in
    /// order (to the discard pile instead of a full hand), with `AfterCardEnteredCombat` for a
    /// card that had no pile; then `AfterCardChangedPiles` for each card whose pile type
    /// changed.
    pub(crate) fn add_cards_to_pile(
        &mut self,
        cards: &[CardId],
        pile: PileType,
        position: Position,
    ) -> Flow {
        if cards.is_empty() || !self.can_add_to_pile() {
            return Ok(());
        }
        // Cards already in piles, to the bottom of a pile that cannot overflow: one move per
        // card would call no hook (no `AfterCardEnteredCombat`) and draw no random number, so
        // the batch moves at once (`docs/semantics/combat-core.md`, "Batch pile moves").
        let mut moved = MovedCards::new();
        let batched = position == Position::Bottom
            && pile != PileType::Hand
            && self.s.move_to_pile_bottom(cards, pile, &mut moved);
        if !batched {
            moved.reserve(cards.len());
            for &card in cards {
                let old = self.move_to_pile(card, pile, position)?;
                moved.push((card, old));
            }
        }
        // A pass that visits nobody changes nothing: when nobody listens now, nobody listens
        // after any card's (empty) pass either.
        if !self.lacks_after_card_changed_piles() {
            for &(card, old) in &moved {
                self.after_card_moved(card, old)?;
            }
        }
        Ok(())
    }

    /// `CardCmd.Exhaust` (`CardCmd.cs:237-248`): a no-op once combat is over or ending; else
    /// to the exhaust pile, then `AfterCardExhausted`.
    pub(crate) fn exhaust(&mut self, card: CardId, caused_by_ethereal: bool) -> Flow {
        if self.hooked().is_over_or_ending() {
            return Ok(());
        }
        self.add_to_pile(card, PileType::Exhaust, Position::Bottom)?;
        self.record(self.s.player.creature, HistoryKind::CardExhausted(card));
        self.after_card_exhausted(card, caused_by_ethereal)
    }

    /// `CardPileCmd.RemoveFromCombat(cards)` (`CardPileCmd.cs:110-206`): each card leaves its
    /// combat pile, in order; then `AfterCardChangedPiles` for each with its old pile, and it
    /// leaves the state (`RemoveFromState`: it stays in [`CombatState::cards`] in no pile). Its
    /// deck version stays in the deck; [`Engine::remove_from_deck`] removes that.
    ///
    /// # Panics
    /// If a card is in no combat pile (`InvalidOperationException` in the game).
    pub(crate) fn remove_from_combat(&mut self, cards: &[CardId]) -> Flow {
        let mut old_piles = Vec::with_capacity(cards.len());
        for &card in cards {
            let old = self
                .s
                .remove_from_pile(card)
                .expect("a card removed from combat is in a combat pile");
            old_piles.push((card, old));
        }
        for (card, old) in old_piles {
            self.after_card_changed_piles(card, Some(old))?;
        }
        Ok(())
    }

    /// `CardPileCmd.RemoveFromDeck(card.DeckVersion)` during a fight (`CardPileCmd.cs:53-89`,
    /// `SwipePower.Steal`): the deck card at `deck_index` (a combat card's `deck_version`) leaves
    /// the run deck. The fight still sees it in [`CombatState::deck`]; the run removes it when
    /// the fight ends, with `BeforeCardRemoved` (run listeners only: no in-scope relic overrides
    /// it, so running it then is the same). The history's `CardsRemoved` and `MarkLootStolen`
    /// are not modelled.
    ///
    /// # Panics
    /// If the card already left the deck (`card.Pile.Type != Deck` throws in the game).
    pub(crate) fn remove_from_deck(&mut self, deck_index: usize) {
        assert!(
            deck_index < self.s.deck.len() && !self.s.removed_from_deck.contains(&deck_index),
            "a card removed from the deck is in the deck"
        );
        self.s.removed_from_deck.push(deck_index);
    }

    /// `CardPileCmd.AddToCombatAndPreview<T>(target, pile, count, creator)`
    /// (`CardPileCmd.cs:1222-1240`): unless the target's player is dead, create `count` new
    /// cards (`CombatState.CreateCard`, which runs `AfterCreated`) and add each with
    /// `AddGeneratedCardToCombat`. `target.Player ?? target.PetOwner` is the only player, so
    /// `_target` picks nothing; `creator` is the creating player's creature, if any.
    pub(crate) fn add_to_combat_and_preview(
        &mut self,
        _target: CreatureId,
        kind: CardKind,
        pile: PileType,
        count: u32,
        creator: Option<CreatureId>,
    ) -> Flow {
        if self.player_dead() {
            return Ok(());
        }
        for _ in 0..count {
            let id = self.create_card(kind);
            self.add_generated_card_to_combat(id, pile, creator)?;
        }
        Ok(())
    }

    /// `CombatState.CreateCard` (`CombatState.cs:174-181`): a new combat card in no pile, then
    /// its `AfterCreated`.
    pub(crate) fn create_card(&mut self, kind: CardKind) -> CardId {
        let id = self.next_card_id();
        self.s.push_card(Card::new(id, kind, 0));
        cards::after_created(self.s, id);
        id
    }

    /// `CardModel.CreateClone` (`CardModel.cs:2182-2193`, `CombatState.CloneCard`): a new
    /// combat card in no pile with the original's upgrade level, keywords and energy cost
    /// state, but without `ExhaustOnNextPlay`, and with `CloneOf` set. No `AfterCreated`. Add it with
    /// [`Engine::add_generated_card_to_combat`].
    pub(crate) fn create_clone(&mut self, original: CardId) -> CardId {
        let id = self.next_card_id();
        let mut clone = *self.s.card(original);
        self.s.cost_lists.copy(original, id);
        clone.id = id;
        clone.pile = None;
        clone.clone_of = Some(original);
        clone.deck_slot = None;
        clone.exhaust_on_next_play = false;
        self.s.push_card(clone);
        id
    }

    /// `CardModel.CreateDupe` (`CardModel.cs:2215-2226`): a dupe of a dupe is one of its
    /// original; else [`Engine::create_clone`] with `IsDupe` set and `Exhaust` removed. A dupe
    /// goes nowhere after its play ([`super::super::card_play`]) and is removed from combat if it
    /// is not played.
    pub(crate) fn create_dupe(&mut self, original: CardId) -> CardId {
        let card = self.s.card(original);
        if card.is_dupe {
            let of = card.clone_of.expect("a dupe is a clone");
            return self.create_dupe(of);
        }
        let dupe = self.create_clone(original);
        let card = self.s.card_mut(dupe);
        card.is_dupe = true;
        card.keywords.remove(CardKeyword::Exhaust);
        dupe
    }

    fn next_card_id(&self) -> CardId {
        CardId(u32::try_from(self.s.cards.len()).expect("card count fits u32"))
    }

    /// `CardCmd.Upgrade(card)` (`CardCmd.cs:256-300`): nothing once combat is ending or for a
    /// card that cannot be upgraded further; else `UpgradeInternal`. No hooks.
    pub(crate) fn upgrade_card(&mut self, card: CardId) {
        if self.hooked().is_ending() {
            return;
        }
        if self.s.card(card).is_upgradable() {
            self.s.upgrade_card_internal(card);
        }
    }

    /// `CardCmd.ApplyKeyword(card, keyword)`.
    pub(crate) fn apply_keyword(&mut self, card: CardId, keyword: CardKeyword) {
        self.s.card_mut(card).keywords.insert(keyword);
    }

    /// `CardCmd.ApplySingleTurnRetain(card)`: `GiveSingleTurnRetain`, cleared by
    /// `EndOfTurnCleanup`.
    pub(crate) fn apply_single_turn_retain(&mut self, card: CardId) {
        self.s.card_mut(card).single_turn_retain = true;
    }

    /// `CardCmd.ApplySingleTurnSly(card)`: `GiveSingleTurnSly`, cleared by `EndOfTurnCleanup`.
    pub(crate) fn apply_single_turn_sly(&mut self, card: CardId) {
        self.s.card_mut(card).single_turn_sly = true;
    }

    /// `CardCmd.RemoveKeyword(card, keyword)`.
    #[expect(
        dead_code,
        reason = "no ported card removes a keyword in combat yet (upgrades use `Card::keywords`)"
    )]
    pub(crate) fn remove_keyword(&mut self, card: CardId, keyword: CardKeyword) {
        self.s.card_mut(card).keywords.remove(keyword);
    }

    /// `PlayerCombatState.EndOfTurnCleanup`: [`Card::end_of_turn_cleanup`] for every card in a
    /// combat pile.
    pub(crate) fn end_of_turn_cleanup(&mut self) {
        // The cards in piles are the cards whose `pile` is set ([`Card::pile`] follows the
        // piles), and each card's cleanup is its own: one pass over the card list.
        debug_assert_eq!(
            self.s.cards.iter().filter(|c| c.pile.is_some()).count(),
            PileType::LISTENER_ORDER
                .iter()
                .map(|&p| self.s.player.pile(p).len())
                .sum::<usize>(),
            "a card is in a pile iff its pile is set"
        );
        for card in &mut self.s.cards {
            if card.pile.is_some() {
                card.end_of_turn_cleanup();
            }
        }
        let cards = &self.s.cards;
        self.s
            .cost_lists
            .end_of_turn_cleanup(|c| cards[c.index()].pile.is_some());
    }

    /// `CardPileCmd.AddGeneratedCardsToCombat` for one card (`CardPileCmd.cs:281-313`): nothing
    /// outside a live combat; else `Add`, then `AfterCardGeneratedForCombat`.
    pub(crate) fn add_generated_card_to_combat(
        &mut self,
        card: CardId,
        pile: PileType,
        creator: Option<CreatureId>,
    ) -> Flow {
        self.add_generated_card_to_combat_at(card, pile, creator, Position::Bottom)
    }

    /// `CardPileCmd.AddGeneratedCardsToCombat(cards, pile, creator, position)`: the cards were
    /// all created beforehand; each is then added in order with
    /// [`Engine::add_generated_card_to_combat_at`].
    pub(crate) fn add_generated_cards_to_combat(
        &mut self,
        cards: &[CardId],
        pile: PileType,
        creator: Option<CreatureId>,
        position: Position,
    ) -> Flow {
        for &card in cards {
            self.add_generated_card_to_combat_at(card, pile, creator, position)?;
        }
        Ok(())
    }

    /// [`Engine::add_generated_card_to_combat`] with an explicit `CardPilePosition`.
    pub(crate) fn add_generated_card_to_combat_at(
        &mut self,
        card: CardId,
        pile: PileType,
        creator: Option<CreatureId>,
        position: Position,
    ) -> Flow {
        if !self.s.in_progress {
            return Ok(());
        }
        self.record(
            self.s.player.creature,
            HistoryKind::CardGenerated { card, creator },
        );
        self.add_to_pile(card, pile, position)?;
        self.after_card_generated_for_combat(card, creator)
    }

    /// `CardPileCmd.Draw(count)` (`CardPileCmd.cs:1010-1070`): `ShouldDraw`, then
    /// `Ceiling(count)` draws from the top (index 0), reshuffling the discard pile into an
    /// empty draw pile, each followed by `AfterCardDrawn`.
    pub(crate) fn draw(&mut self, count: Decimal, from_hand_draw: bool) -> Flow<Cards> {
        let mut drawn = Cards::new();
        if self.hooked().is_over_or_ending() {
            return Ok(drawn);
        }
        if let Some(preventer) = self.hooked().draw_preventer(from_hand_draw) {
            self.after_preventing_draw(preventer)?;
            return Ok(drawn);
        }
        let requested = if count > Decimal::ZERO {
            count.ceiling().to_int_truncating()
        } else {
            0
        };
        for _ in 0..requested {
            if self.s.player.hand.len() >= MAX_HAND_SIZE || self.hooked().is_over_or_ending() {
                break;
            }
            if !self.draw_is_possible() {
                break;
            }
            self.check_hidden()?;
            let shuffle = self.s.player.draw.is_empty() && !self.s.player.discard.is_empty();
            if shuffle {
                self.shuffle()?;
            }
            if !self.draw_is_possible() {
                break;
            }
            let Some(&card) = self.s.player.draw.first() else {
                break;
            };
            if self.s.player.hand.len() >= MAX_HAND_SIZE {
                break;
            }
            drawn.push(card);
            if shuffle {
                self.add_to_pile(card, PileType::Hand, Position::Bottom)?;
            } else if !self.player_dead() {
                // `add_to_pile`: nothing changed since the loop's `is_over_or_ending` test, which
                // is `can_add_to_pile` without the owner's death.
                if cfg!(debug_assertions) {
                    let allowed = self.can_add_to_pile();
                    assert!(allowed, "the draw's own test is the add's");
                }
                // `add_allowed_to_pile(card, Hand, Bottom)` for the draw pile's top card and a
                // hand with room: the card had a pile, so no `AfterCardEnteredCombat`.
                self.s.draw_top_to_hand(card);
                self.after_card_moved(card, Some(PileType::Draw))?;
            }
            self.emit(Event::CardDrawn(card));
            let kind = HistoryKind::CardDrawn {
                card,
                from_hand_draw,
            };
            self.record(self.s.player.creature, kind);
            self.after_card_drawn(card, from_hand_draw)?;
        }
        Ok(drawn)
    }

    /// `CardPileCmd.AutoPlayFromDrawPile(player, count, position, forceExhaust)`
    /// (`CardPileCmd.cs:1148-1183`): nothing once combat is over or ending. Up to `count` times,
    /// `ShuffleIfNecessary`, then take the draw pile's top or bottom card (stop when it
    /// is empty) to the Play pile; then auto-play each taken card with no target, its
    /// `ExhaustOnNextPlay` set to `force_exhaust`, until the player dies.
    pub(crate) fn auto_play_from_draw_pile(
        &mut self,
        count: u32,
        position: Position,
        force_exhaust: bool,
    ) -> Flow {
        if self.hooked().is_over_or_ending() {
            return Ok(());
        }
        let mut cards = Vec::new();
        for _ in 0..count {
            self.shuffle_if_necessary()?;
            let s = &mut *self.s;
            let draw = &s.player.draw;
            let card = match position {
                Position::Bottom => draw.last().copied(),
                Position::Top => draw.first().copied(),
                Position::Random => s.rng[RunRngType::CombatCardSelection]
                    .next_item(draw)
                    .copied(),
            };
            let Some(card) = card else {
                break;
            };
            cards.push(card);
            self.add_to_pile(card, PileType::Play, Position::Bottom)?;
        }
        for card in cards {
            if self.player_dead() {
                break;
            }
            self.s.card_mut(card).exhaust_on_next_play = force_exhaust;
            self.auto_play(card, None)?;
        }
        Ok(())
    }

    /// `CardPileCmd.ShuffleIfNecessary` (`CardPileCmd.cs:1189-1198`): shuffle when the draw
    /// pile is empty and the discard pile is not.
    pub(crate) fn shuffle_if_necessary(&mut self) -> Flow {
        if self.s.player.draw.is_empty() && !self.s.player.discard.is_empty() {
            self.shuffle()?;
        }
        Ok(())
    }

    /// `CheckIfDrawIsPossible...`: some card left in the draw or discard pile.
    fn draw_is_possible(&self) -> bool {
        !self.s.player.draw.is_empty() || !self.s.player.discard.is_empty()
    }

    /// `CardPileCmd.Shuffle` (`CardPileCmd.cs:1076-1136`): discard ++ draw, `StableShuffle` on
    /// `Rng.Shuffle` (sorted by `CardModel.CompareTo` first), then one batch to the draw pile
    /// bottom, then `AfterShuffle`.
    pub(crate) fn shuffle(&mut self) -> Flow {
        if self.hooked().is_over_or_ending() {
            return Ok(());
        }
        self.check_hidden()?;
        let mut list = self.s.player.discard.to_vec();
        list.extend(self.s.player.draw.iter().copied());
        let state = &mut *self.s;
        let all_cards = &state.cards;
        stable_shuffle_by_key(&mut list, &mut state.rng[RunRngType::Shuffle], |c| {
            cards::sort_key(&all_cards[c.index()])
        });
        self.hooked().modify_shuffle_order(&mut list, false);
        self.add_cards_to_pile(&list, PileType::Draw, Position::Bottom)?;
        self.emit(Event::Shuffled);
        if !self.hooked().is_over_or_ending() {
            self.after_shuffle()?;
        }
        Ok(())
    }

    /// `FlushPlayerHand` (`CombatManager.cs:1785-1820`): unless the player is dead,
    /// `ShouldFlush`, then every hand card that is not retained this turn to the discard
    /// bottom as one batch (all are kept if a listener vetoes), `AfterFlush`,
    /// `EndOfTurnCleanup`.
    pub(crate) fn flush_hand(&mut self) -> Flow {
        if self.player_dead() {
            return Ok(());
        }
        let hooked = self.hooked();
        let should_flush = hooked.should_flush();
        let (retained, flushed): (Cards, Cards) = hooked
            .s
            .player
            .hand
            .iter()
            .copied()
            .partition(|&c| !should_flush || hooked.should_retain_this_turn(c));
        self.add_cards_to_pile(&flushed, PileType::Discard, Position::Bottom)?;
        if self.records_events() {
            self.emit(Event::HandFlushed(flushed.to_vec()));
        }
        self.after_flush(&flushed, &retained)?;
        self.end_of_turn_cleanup();
        Ok(())
    }
}

/// A random place in `pile` (`Rng.Shuffle.NextInt(cardPile.Cards.Count + 1)`), drawn after the
/// card left its old pile; it draws even for an empty pile. Out of line: few moves are random,
/// and the common path keeps fewer registers without it.
#[cold]
#[inline(never)]
fn random_pile_index(s: &mut CombatState, pile: PileType) -> usize {
    let count = s.player.pile(pile).len();
    let bound = i32::try_from(count + 1).expect("pile size fits i32");
    let index = s.rng[RunRngType::Shuffle].next_int(bound);
    usize::try_from(index).expect("NextInt is non-negative")
}
