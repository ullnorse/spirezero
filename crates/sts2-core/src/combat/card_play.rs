//! The card-play pipeline: `PlayCardAction.ExecuteAction` and `CardModel.OnPlayWrapper`
//! (`PlayCardAction.cs:62-104`, `CardModel.cs:1858-2019`).

use super::commands::Position;
use super::engine::{Engine, Event, Flow};
use super::history::{HistoryKind, HistoryTag};
use super::state::{CardId, CreatureId, PileType};
use crate::content::cards::{self, CardKeyword, CardType, TargetType};
use crate::decimal::Decimal;
use crate::rng::RunRngType;

/// The pile a played card goes to afterwards (`CardPlay.ResultPile`, the `pileType` of
/// `GetResultLocationForCardPlay` after `ModifyCardPlayResultLocation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResultPile {
    /// Removed from combat (powers).
    None,
    /// `ResultPile.Exhaust`.
    Exhaust,
    /// `ResultPile.Discard`.
    Discard,
    /// Back into the draw pile (`NostalgiaPower`, `ShiningStrike`).
    Draw,
    /// Back into the hand (`ParticleWall`); to the discard pile instead if the hand is full.
    Hand,
}

/// `CardLocation` of a card play's result: the pile and the position in it. The player is
/// always the card's owner (no in-scope listener gives a card to another player).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResultLocation {
    pub pile: ResultPile,
    pub position: Position,
}

/// `CardPlay`: one play of a card, as passed to `OnPlay` and the card-play hooks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CardPlay {
    /// The play's identity (the game compares `CardPlay` objects by reference): the number of
    /// plays started earlier this combat.
    pub id: u32,
    /// The card played.
    pub card: CardId,
    /// `Target`.
    pub target: Option<CreatureId>,
    /// `ResultPile`.
    pub result_pile: ResultPile,
    /// `IsAutoPlay`.
    pub is_auto_play: bool,
    /// 0-based index of this play among `play_count`.
    pub play_index: u32,
    /// `PlayCount`: how many times the card is played this time.
    pub play_count: u32,
    /// `Resources.EnergyValue`: the energy the card cost when played. For a manual play that is
    /// the energy spent; an auto-play spends nothing but still has the cost it would have had.
    pub energy_value: i32,
    /// `Resources.StarValue`: the stars the card cost when played, as `energy_value`.
    pub star_value: i32,
}

impl CardPlay {
    /// `IsFirstInSeries`.
    #[must_use]
    pub const fn is_first_in_series(&self) -> bool {
        self.play_index == 0
    }

    /// `IsLastInSeries`.
    #[must_use]
    pub const fn is_last_in_series(&self) -> bool {
        self.play_index + 1 == self.play_count
    }

    /// `Resources.EnergySpent`: 0 for an auto-play.
    #[must_use]
    pub const fn energy_spent(&self) -> i32 {
        if self.is_auto_play {
            0
        } else {
            self.energy_value
        }
    }

    /// `Resources.StarsSpent`: 0 for an auto-play.
    #[must_use]
    pub const fn stars_spent(&self) -> i32 {
        if self.is_auto_play {
            0
        } else {
            self.star_value
        }
    }
}

/// `ResourceInfo`'s values of a play: `EnergyValue` and `StarValue` (what the card cost when
/// played). `EnergySpent` and `StarsSpent` are these for a manual play and 0 for an auto-play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resources {
    pub energy: i32,
    pub stars: i32,
}

/// `AutoPlayType` (`Entities.Cards/AutoPlayType.cs`): how a card is played, as `ShouldPlay` and
/// `BeforeCardAutoPlayed` see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AutoPlayType {
    /// A manual play (`CardModel.CanPlay`).
    None,
    /// `CardCmd.AutoPlay`'s default.
    Default,
    /// A Sly card discarded (`CardCmd.Discard`).
    SlyDiscard,
}

impl Engine<'_> {
    /// A manual card play, from spending energy to the executor's `CheckWinCondition`.
    /// The caller has checked that the play is legal (`CanPlay`, `IsValidTarget`).
    pub(crate) fn play_card_action(&mut self, card: CardId, target: Option<CreatureId>) -> Flow {
        let resources = self.spend_resources(card)?;
        self.on_play_wrapper(card, target, false, resources)?;
        self.check_win_condition()?;
        Ok(())
    }

    /// `CardModel.SpendResources` (`CardModel.cs:1807-1845`): both amounts first
    /// (`GetAmountToSpend`, `max(0, GetStarCostWithModifiers)`; `ShouldPayExcessEnergyCostWithStars`
    /// has no listener). `SpendEnergy`: captured as X for an X cost, lost (and logged) only if
    /// positive, but `AfterEnergySpent` fires even for 0. `SpendStars`: `LastStarsSpent` always,
    /// lost (and logged) and `AfterStarsSpent` only if positive.
    pub(crate) fn spend_resources(&mut self, card: CardId) -> Flow<Resources> {
        let amount = self.hooked().energy_to_spend(card);
        let stars = self.hooked().star_cost(card).max(0);
        let cost = &mut self.s.card_mut(card).energy_cost;
        if cost.costs_x {
            cost.captured_x = amount;
        }
        if amount > 0 {
            self.record(self.s.player.creature, HistoryKind::EnergySpent(amount));
            self.s.player.energy -= amount;
        }
        self.after_energy_spent(card, amount)?;
        self.s.card_mut(card).star_cost.last_spent = stars;
        if stars > 0 {
            self.lose_stars_internal(Decimal::from(stars));
            self.after_stars_spent(stars)?;
        }
        Ok(Resources {
            energy: amount,
            stars,
        })
    }

    /// `CardCmd.AutoPlay(card, target)` with `AutoPlayType.Default` (`CardCmd.cs:51-131`):
    /// play `card` without paying for it. An Unplayable card or a `ShouldPlay` veto sends it
    /// to its result pile unplayed; an `AnyEnemy` card with no target picks one on
    /// `Rng.CombatTargets` among the hittable enemies.
    pub(crate) fn auto_play(&mut self, card: CardId, target: Option<CreatureId>) -> Flow {
        self.auto_play_with(card, target, AutoPlayType::Default, false)
    }

    /// [`Self::auto_play`] as `auto_play` (`ShouldPlay` sees it: `Enthralled` only vetoes
    /// manual plays) and with `skipXCapture`: the caller already spent the resources
    /// ([`Self::spend_resources`]), which captured X and the stars spent.
    pub(crate) fn auto_play_with(
        &mut self,
        card: CardId,
        target: Option<CreatureId>,
        auto_play: AutoPlayType,
        skip_x_capture: bool,
    ) -> Flow {
        if self.hooked().is_over_or_ending() || self.player_dead() {
            return Ok(());
        }
        if self.hooked().has_keyword(card, CardKeyword::Unplayable)
            || !self.hooked().should_play(card, auto_play)
        {
            return self.move_to_result_pile_without_playing(card);
        }
        let mut target = target;
        if self.s.target_type(card) == TargetType::AnyEnemy && target.is_none() {
            let hittable = self.hooked().hittable_enemies();
            target = self.s.rng[RunRngType::CombatTargets]
                .next_item(&hittable)
                .copied();
            if target.is_none() {
                return self.move_to_result_pile_without_playing(card);
            }
        }
        if !skip_x_capture {
            let energy = self.s.player.energy;
            let cost = &mut self.s.card_mut(card).energy_cost;
            if cost.costs_x {
                cost.captured_x = energy;
            }
            // `LastStarsSpent`: all stars for an X star cost, else the cost (before the card has a
            // pile, so without `ModifyStarCost` for a card in none).
            let last_spent = if self.s.card(card).star_cost.costs_x {
                self.s.player.stars
            } else {
                self.hooked().star_cost(card).max(0)
            };
            self.s.card_mut(card).star_cost.last_spent = last_spent;
        }
        if self.s.pile_of(card).is_none() {
            self.add_to_pile(card, PileType::Play, Position::Bottom)?;
        }
        // Hook.BeforeCardAutoPlayed: no in-scope listener.
        // ResourceInfo: `EnergyValue = EnergyCost.GetAmountToSpend()`, after the X capture;
        // `StarValue = max(0, GetStarCostWithModifiers())`.
        let resources = Resources {
            energy: self.hooked().energy_to_spend(card),
            stars: self.hooked().star_cost(card).max(0),
        };
        self.on_play_wrapper(card, target, true, resources)
    }

    /// `CardCmd.MoveToResultPileWithoutPlaying` (`CardCmd.cs:133`,
    /// `CardModel.MoveToResultPileWithoutPlaying`): to the Play pile, then (if still there) a
    /// dupe is removed from combat, any other card exhausted or discarded; unlike a play, a
    /// power card is discarded too.
    fn move_to_result_pile_without_playing(&mut self, card: CardId) -> Flow {
        self.add_to_pile(card, PileType::Play, Position::Bottom)?;
        if self.s.pile_of(card) != Some(PileType::Play) {
            return Ok(());
        }
        if self.s.card(card).is_dupe {
            return self.remove_from_combat(&[card]);
        }
        if self.s.card(card).exhaust_on_next_play
            || self.hooked().has_keyword(card, CardKeyword::Exhaust)
        {
            self.exhaust(card, false)
        } else {
            self.add_to_pile(card, PileType::Discard, Position::Bottom)
        }
    }

    /// `CardModel.OnPlayWrapper` (`CardModel.cs:1858-2019`).
    fn on_play_wrapper(
        &mut self,
        card: CardId,
        target: Option<CreatureId>,
        is_auto_play: bool,
        resources: Resources,
    ) -> Flow {
        if is_auto_play {
            self.add_to_pile(card, PileType::Play, Position::Bottom)?;
        } else if !self.hooked().is_over_or_ending() {
            // AddDuringManualCardPlay (`CardPileCmd.cs:830-866`): hand to the Play pile,
            // without the full-hand redirect, then AfterCardChangedPiles.
            let old = self.s.move_card(card, PileType::Play, |_| None);
            self.after_card_changed_piles(card, old)?;
        }
        self.emit(Event::CardPlayed { card, target });

        // GetResultLocationForCardPlay (`CardModel.cs:2084`), decided before OnPlay; it
        // consumes ExhaustOnNextPlay. A dupe or a power goes nowhere. The base location goes
        // through the card class's override (each calls `base` first).
        let exhaust = self.hooked().has_keyword(card, CardKeyword::Exhaust);
        let c = self.s.card_mut(card);
        let pile = if c.is_dupe || c.card_type() == CardType::Power {
            ResultPile::None
        } else if c.exhaust_on_next_play || exhaust {
            c.exhaust_on_next_play = false;
            ResultPile::Exhaust
        } else {
            ResultPile::Discard
        };
        let kind = c.kind;
        let location = cards::result_location_for_card_play(
            kind,
            ResultLocation {
                pile,
                position: Position::Bottom,
            },
        );
        let (location, modifiers) =
            self.hooked()
                .modify_card_play_result_location(card, is_auto_play, resources, location);
        self.after_modifying_card_play_result_location(card, location, &modifiers)?;
        let result_pile = location.pile;
        // GeneratePlayCount: `GetEnchantedReplayCount() + 1` before the hook
        // (`CardModel.cs:1132`: the enchantment's `EnchantPlayCount(BaseReplayCount)`).
        let c = self.s.card(card);
        let replays = c.enchantment.map_or(c.base_replay_count, |e| {
            e.kind.enchant_play_count(e, c.base_replay_count)
        });
        let (play_count, modifiers) =
            self.hooked()
                .modify_card_play_count(card, target, replays + 1);
        self.after_modifying_card_play_count(card, &modifiers)?;
        if self.player_dead() {
            return Ok(());
        }

        self.s.player.effect_depth += 1;
        let finished = self.play_loop(
            card,
            target,
            result_pile,
            is_auto_play,
            play_count,
            resources,
        );
        self.s.player.effect_depth -= 1;
        if !finished? {
            return Ok(());
        }

        if self.s.pile_of(card) == Some(PileType::Play) {
            match result_pile {
                ResultPile::None => {
                    self.s.remove_from_pile(card);
                }
                ResultPile::Exhaust => self.exhaust(card, false)?,
                ResultPile::Discard => {
                    self.add_to_pile(card, PileType::Discard, location.position)?;
                }
                ResultPile::Draw => self.add_to_pile(card, PileType::Draw, location.position)?,
                ResultPile::Hand => self.add_to_pile(card, PileType::Hand, location.position)?,
            }
        }
        self.check_for_empty_hand()?;
        self.s.cost_lists.after_card_played_cleanup(card);
        Ok(())
    }

    /// The `playCount` loop. Returns `false` when the owner died (the wrapper then returns
    /// without moving the card).
    fn play_loop(
        &mut self,
        card: CardId,
        target: Option<CreatureId>,
        result_pile: ResultPile,
        is_auto_play: bool,
        play_count: u32,
        resources: Resources,
    ) -> Flow<bool> {
        let me = self.s.player.creature;
        for play_index in 0..play_count {
            if self.hooked().is_over_or_ending() {
                break;
            }
            let id = self.s.history_count(HistoryTag::CardPlayStarted);
            // Built twice from the same values: the history's copy is then stored from
            // registers, not reloaded from `play` right after its stores (which stalls).
            let new_play = || CardPlay {
                id,
                card,
                target,
                result_pile,
                is_auto_play,
                play_index,
                play_count,
                energy_value: resources.energy,
                star_value: resources.stars,
            };
            let play = new_play();
            self.before_card_played(&play)?;
            self.record(me, HistoryKind::CardPlayStarted(new_play()));
            cards::on_play(self, &play)?;
            if self.player_dead() {
                return Ok(false);
            }
            // Enchantment.OnPlay (`CardModel.cs:1940`), then Affliction.OnPlay: none in scope.
            if let Some(enchantment) = self.s.card(card).enchantment {
                enchantment.kind.model().on_play(self, card, &play)?;
                if self.player_dead() {
                    return Ok(false);
                }
            }
            let was_ethereal = self.hooked().has_keyword(card, CardKeyword::Ethereal);
            let finished = HistoryKind::CardPlayFinished {
                play,
                card_type: self.s.card(card).card_type(),
                was_ethereal,
            };
            self.record(me, finished);
            if self.s.in_progress {
                self.after_card_played(&play)?;
            }
            if self.player_dead() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn player_dead(&self) -> bool {
        self.s.creature(self.s.player.creature).is_dead()
    }
}
