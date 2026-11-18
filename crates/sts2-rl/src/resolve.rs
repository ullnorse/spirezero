//! Combat resolvers for the run env's hybrid mode (`docs/reviews/spire-mentor.md`, idea 1): the
//! agent makes the run-level choices and a [`Resolver`] plays each fight inside the env.
//!
//! [`Greedy`] is the sweep's greedy combat policy (`cargo xtask sweep --policy greedy`, which
//! `sts2-diff` drives through this module): each turn is planned by a small beam search over play
//! sequences, each scored by ending the turn on a copy (damage dealt, enemies killed, debuffs, HP
//! lost, Osty's health); potions only when low (or in elite and boss fights when hurt), card
//! choices take the first options. [`Random`] plays uniformly random legal actions. Both only
//! ever pick from the legal list they are given, and both are deterministic given the run's seed.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::{fmt, mem};

use sts2_core::Combat;
use sts2_core::combat::{Action, CombatState, Decision, DecisionRequest, Side};
use sts2_core::content::cards::CardType;
use sts2_core::content::powers::PowerType;
use sts2_core::run::{Room, RoomType, Run, RunAction, RunDecision};

use crate::seeds::SplitMix;

/// Potions are used below this share of max HP (in percent).
const POTION_HP_PERCENT: i32 = 40;
/// In an elite or boss fight potions are used below this share of max HP (in percent).
const BIG_FIGHT_POTION_HP_PERCENT: i32 = 75;
/// What a point of the player's HP is worth, against 10 per point of enemy HP.
const HP_WEIGHT: i64 = 15;
/// What playing a power is worth beyond what the turn shows.
const POWER_BONUS: i64 = 25;
/// What a potion is worth beyond its effect when it may be used.
const POTION_BONUS: i64 = 20;
/// The most plays a turn plan holds.
pub(crate) const MAX_PLAYS: usize = 10;
/// How many partial plans the search keeps at each depth.
const BEAM: usize = 3;
/// Card plays in one turn before [`Random`] only ends the turn (zero-cost loops), as the run
/// bench's random policy.
const MAX_RANDOM_PLAYS_PER_TURN: usize = 40;

/// Plays the fights of a run: asked for one action at a time while the run is in a combat.
pub trait Resolver: fmt::Debug + Send {
    /// A new run on `seed` begins.
    fn reset(&mut self, seed: u64);

    /// The next action of `run`'s fight, from `legal` (its legal list, non-empty, every entry a
    /// [`RunAction::Combat`]). It may also answer a pending card choice with a whole
    /// [`Decision::Cards`] the list does not show.
    fn act(&mut self, run: &Run, legal: &[RunAction]) -> RunAction;
}

/// How the run env plays combat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CombatMode {
    /// The agent plays combat through the combat action space.
    #[default]
    Agent,
    /// [`Greedy`] settles every fight inside the env.
    Greedy,
    /// [`Random`] settles every fight inside the env.
    Random,
}

impl CombatMode {
    /// The modes by their ABI code (0 agent, 1 greedy, 2 random).
    pub const ALL: [Self; 3] = [Self::Agent, Self::Greedy, Self::Random];

    /// The resolver of the mode; `None` for [`CombatMode::Agent`].
    #[must_use]
    pub fn resolver(self) -> Option<Box<dyn Resolver>> {
        match self {
            Self::Agent => None,
            Self::Greedy => Some(Box::new(Greedy::default())),
            Self::Random => Some(Box::new(Random::default())),
        }
    }

    /// The mode named `name` (`agent`, `greedy`, `random`).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "agent" => Some(Self::Agent),
            "greedy" => Some(Self::Greedy),
            "random" => Some(Self::Random),
            _ => None,
        }
    }
}

/// The greedy combat policy: it keeps the rest of the turn it planned.
#[derive(Clone, Debug, Default)]
pub struct Greedy {
    /// The plays still to make this turn, in order.
    plan: VecDeque<Action>,
}

impl Greedy {
    /// The greedy action in `combat` (the run's fight) among `modeled` (non-empty), the actions
    /// the engine models right now. A card choice is answered with [`Greedy::answer`] instead.
    pub fn choose(&mut self, run: &Run, combat: &Combat, modeled: &[RunAction]) -> RunAction {
        if let Some(next) = self.plan.pop_front() {
            let action = RunAction::Combat(next);
            if modeled.contains(&action) {
                return action;
            }
        }
        let big = matches!(
            run.state().room,
            Room::Combat {
                room_type: RoomType::Elite | RoomType::Boss,
                ..
            }
        );
        let first: Vec<Action> = modeled
            .iter()
            .filter_map(|a| match a {
                RunAction::Combat(a) => Some(a.clone()),
                _ => None,
            })
            .collect();
        self.plan = plan_turn(combat, big, &first).into();
        let end_turn = RunAction::Combat(Action::EndTurn);
        match self.plan.pop_front() {
            Some(a) => RunAction::Combat(a),
            None if modeled.contains(&end_turn) => end_turn,
            None => modeled[0].clone(),
        }
    }

    /// Forgets the planned turn (the fight is over).
    pub fn clear(&mut self) {
        self.plan.clear();
    }

    /// The answer to a card choice inside combat: the first options, as the plan assumed.
    #[must_use]
    pub fn answer(combat: &Combat) -> Option<Action> {
        whole_choice(combat, |_| 0)
    }
}

impl Resolver for Greedy {
    fn reset(&mut self, _seed: u64) {
        self.clear();
    }

    fn act(&mut self, run: &Run, legal: &[RunAction]) -> RunAction {
        let RunDecision::Combat(combat) = run.pending_decision() else {
            self.clear();
            return legal[0].clone();
        };
        if combat.pending_decision().is_some()
            && let Some(answer) = Self::answer(combat)
        {
            return RunAction::Combat(answer);
        }
        self.choose(run, combat, legal)
    }
}

/// Uniformly random legal actions from a `SplitMix64` seeded by the run's seed; after
/// [`MAX_RANDOM_PLAYS_PER_TURN`] card plays in a turn it only ends the turn or does what
/// else is legal.
#[derive(Clone, Debug, Default)]
pub struct Random {
    rng: SplitMix,
    plays: usize,
}

impl Resolver for Random {
    fn reset(&mut self, seed: u64) {
        self.rng = SplitMix::new(seed ^ 0x5245_534F_4C56_4552);
        self.plays = 0;
    }

    fn act(&mut self, _run: &Run, legal: &[RunAction]) -> RunAction {
        let is_play = |a: &RunAction| matches!(a, RunAction::Combat(Action::PlayCard { .. }));
        let capped = self.plays >= MAX_RANDOM_PLAYS_PER_TURN && legal.iter().any(|a| !is_play(a));
        let choices: Vec<&RunAction> = legal.iter().filter(|a| !capped || !is_play(a)).collect();
        let action = choices[self.rng.below(choices.len())].clone();
        self.plays = match action {
            RunAction::Combat(Action::PlayCard { .. }) => self.plays + 1,
            RunAction::Combat(Action::UsePotion { .. } | Action::Resolve(_)) => self.plays,
            _ => 0,
        };
        action
    }
}

/// For a pending card choice, the whole selection as one action (how the referee takes it).
///
/// The sequential picks and `Done` of [`Combat::legal_actions`] are made on a copy, each chosen
/// by `pick(number of legal actions)`. `None` when no card choice is pending.
///
/// # Panics
/// If the engine rejects one of its own legal actions.
pub fn whole_choice(combat: &Combat, mut pick: impl FnMut(usize) -> usize) -> Option<Action> {
    combat.pending_decision()?;
    let mut copy = combat.clone();
    let mut cards = Vec::new();
    loop {
        let legal = copy.legal_actions();
        let action = legal[pick(legal.len())].clone();
        let Action::Resolve(Decision::Pick(card)) = action else {
            break;
        };
        cards.push(card);
        copy.apply(action).expect("a legal pick");
        let still_choosing = matches!(
            copy.pending_decision(),
            Some(DecisionRequest::ChooseCards { picked, .. }) if *picked == cards
        );
        if !still_choosing {
            break;
        }
    }
    Some(Action::Resolve(Decision::Cards(cards)))
}

/// The plays of this turn, found by a beam search: each candidate sequence of plays is scored
/// by ending the turn after it on a copy and comparing the state at the start of the next player
/// turn with the current one (so block and debuffs count through the damage they prevent).
/// Ending the turn now is the baseline. A winning play ends the search.
fn plan_turn(combat: &Combat, big_fight: bool, first: &[Action]) -> Vec<Action> {
    search_turn(combat.clone(), big_fight, first, MAX_PLAYS, None)
}

/// The score of a play that wins the fight (the planner and the plan column).
pub const WIN_SCORE: i64 = i64::MAX / 2;

/// [`plan_turn`] with plans of at most `max_plays` plays; with `scores` (one per entry of `first`), it also records for each first
/// play the best score of any plan the search tried that starts with it (`None` if it was not
/// tried), and `first`'s [`Action::EndTurn`] gets the baseline. A potion the plan may not drink
/// gets the score of drinking it and ending the turn, but the search does not go on from it,
/// so the plan is the same with or without `scores`.
#[allow(
    clippy::too_many_lines,
    reason = "one beam search loop; splitting it scatters its shared buffers"
)]
pub(crate) fn search_turn(
    combat: Combat,
    big_fight: bool,
    first: &[Action],
    max_plays: usize,
    mut scores: Option<&mut [Option<i64>]>,
) -> Vec<Action> {
    let before = Snapshot::of(combat.state());
    let potions_ok = before.hp * 100 < before.max_hp * POTION_HP_PERCENT
        || (big_fight && before.hp * 100 < before.max_hp * BIG_FIGHT_POTION_HP_PERCENT);
    // Every copy comes from `spare` (refilled with `clone_from`, so the search allocates no
    // combat after its first few) and goes back to it when it is done with.
    let mut spare: Vec<Combat> = Vec::new();
    let mut scratch = combat.clone();
    end_turn_in(&mut scratch);
    let mut best = (before.score_to(&Snapshot::of(scratch.state())), Vec::new());
    if let Some(scores) = scores.as_deref_mut() {
        for (score, a) in scores.iter_mut().zip(first) {
            if *a == Action::EndTurn {
                *score = Some(best.0);
            }
        }
    }
    // Each plan carries the index in `first` of its first play.
    let mut beam: Vec<(Combat, Vec<Action>, i64, usize)> =
        vec![(combat, Vec::new(), 0, usize::MAX)];
    for depth in 0..max_plays {
        // At the last depth the plans are only scored, so their states are not kept.
        let last = depth + 1 == max_plays;
        let mut next: Vec<Scored> = Vec::new();
        for (state, plan, bonus, root) in &beam {
            // The first play must be one the run models; later ones are checked when made.
            let legal = if depth == 0 {
                Cow::Borrowed(first)
            } else {
                Cow::Owned(state.legal_actions())
            };
            for (i, a) in legal.iter().enumerate() {
                let root = if depth == 0 { i } else { *root };
                let potion = matches!(a, Action::UsePotion { .. });
                let held_potion = potion && !potions_ok;
                if matches!(a, Action::EndTurn | Action::Resolve(_))
                    || (held_potion && (depth > 0 || scores.is_none()))
                {
                    continue;
                }
                let power = matches!(a, Action::PlayCard { card, .. }
                    if state.state().card(*card).card_type() == CardType::Power);
                let mut copy = copy_of(&mut spare, state);
                if copy.apply(a.clone()).is_err() {
                    spare.push(copy);
                    continue;
                }
                resolve_choices_in(&mut copy);
                let mut plan = plan.clone();
                plan.push(a.clone());
                if copy.state().result.is_some() && Snapshot::of(copy.state()).hp > 0 {
                    record(scores.as_deref_mut(), root, WIN_SCORE);
                    return plan;
                }
                if held_potion {
                    // Scored for the column only: what drinking it now would do.
                    end_turn_in(&mut copy);
                    let score = before.score_to(&Snapshot::of(copy.state()));
                    record(scores.as_deref_mut(), root, score);
                    spare.push(copy);
                    continue;
                }
                // Spending a card is free unless it makes things worse; a potion is worth
                // something when it may be drunk.
                let bonus = bonus
                    + match (potion, power) {
                        (true, _) => POTION_BONUS,
                        // A power pays off over the fight, which one turn does not show.
                        (_, true) => POWER_BONUS,
                        _ => 1,
                    };
                let after = if last {
                    end_turn_in(&mut copy);
                    &copy
                } else {
                    scratch.clone_from(&copy);
                    end_turn_in(&mut scratch);
                    &scratch
                };
                let score = before
                    .score_to(&Snapshot::of(after.state()))
                    .saturating_add(bonus);
                record(scores.as_deref_mut(), root, score);
                let kept = if last {
                    spare.push(copy);
                    None
                } else {
                    Some(copy)
                };
                next.push((score, kept, plan, bonus, root));
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort_by_key(|(score, ..)| std::cmp::Reverse(*score));
        let cut = BEAM.min(next.len());
        spare.extend(next.drain(cut..).filter_map(|(_, state, ..)| state));
        if next[0].0 > best.0 {
            best = (next[0].0, next[0].2.clone());
        }
        if last {
            break;
        }
        let kept = next
            .into_iter()
            .filter_map(|(_, state, plan, bonus, root)| state.map(|s| (s, plan, bonus, root)))
            .collect();
        spare.extend(mem::replace(&mut beam, kept).into_iter().map(|(s, ..)| s));
    }
    best.1
}

/// A scored plan: its score, its state after the plays (none at the last depth), the plays,
/// their bonus and the index in `first` of the first play.
type Scored = (i64, Option<Combat>, Vec<Action>, i64, usize);

/// A copy of `state`, refilling a spare one if there is one.
fn copy_of(spare: &mut Vec<Combat>, state: &Combat) -> Combat {
    spare.pop().map_or_else(
        || state.clone(),
        |mut copy| {
            copy.clone_from(state);
            copy
        },
    )
}

/// Keeps the best `score` of the plans that start with `first[i]`.
fn record(scores: Option<&mut [Option<i64>]>, i: usize, score: i64) {
    if let Some(slot) = scores.and_then(|s| s.get_mut(i)) {
        *slot = Some(slot.map_or(score, |s| s.max(score)));
    }
}

/// Answers any pending card choice with the first options.
pub(crate) fn resolve_choices(mut combat: Combat) -> Combat {
    resolve_choices_in(&mut combat);
    combat
}

/// [`resolve_choices`] in place.
pub(crate) fn resolve_choices_in(combat: &mut Combat) {
    while combat.pending_decision().is_some() {
        let Some(answer) = whole_choice(combat, |_| 0) else {
            break;
        };
        if combat.apply(answer).is_err() {
            break;
        }
    }
}

/// Finishes any pending card choice with the first options, then ends the turn, on `combat`.
pub(crate) fn end_turn_of(mut combat: Combat) -> Combat {
    end_turn_in(&mut combat);
    combat
}

/// [`end_turn_of`] in place.
pub(crate) fn end_turn_in(combat: &mut Combat) {
    resolve_choices_in(combat);
    if combat.state().result.is_none() && combat.pending_decision().is_none() {
        let _ = combat.apply(Action::EndTurn);
    }
}

/// What the lookahead compares.
pub(crate) struct Snapshot {
    over: bool,
    hp: i32,
    max_hp: i32,
    block: i32,
    /// Osty's (and any pet's) HP plus block.
    pets: i32,
    /// The living enemies' HP plus block.
    enemy_hp: i32,
    /// The number of living enemies.
    enemies: i32,
    /// The total amount of debuffs on living enemies.
    enemy_debuffs: i32,
}

impl Snapshot {
    pub(crate) fn of(s: &CombatState) -> Self {
        let player = s.creature(s.player.creature);
        let enemies: Vec<_> = s
            .enemies
            .iter()
            .map(|&id| s.creature(id))
            .filter(|c| c.is_alive())
            .collect();
        Self {
            over: s.result.is_some(),
            hp: player.hp,
            max_hp: player.max_hp,
            block: player.block,
            pets: s
                .allies
                .iter()
                .map(|&id| s.creature(id))
                .filter(|c| c.side == Side::Player && c.pet_owner.is_some() && c.is_alive())
                .map(|c| c.hp + c.block)
                .sum(),
            enemy_hp: enemies.iter().map(|c| c.hp + c.block).sum(),
            enemies: i32::try_from(enemies.len()).unwrap_or(i32::MAX),
            enemy_debuffs: enemies
                .iter()
                .flat_map(|c| &c.powers)
                .filter(|p| p.kind.power_type() == PowerType::Debuff)
                .map(|p| p.amount.abs())
                .sum(),
        }
    }

    /// How much better `after` is than `self`.
    pub(crate) fn score_to(&self, after: &Self) -> i64 {
        if after.over {
            return if after.hp > 0 {
                i64::MAX / 2
            } else {
                i64::MIN / 2
            };
        }
        let delta = |f: fn(&Self) -> i32| i64::from(f(after) - f(self));
        30 * delta(|s| -s.enemies)
            + 10 * delta(|s| -s.enemy_hp)
            + 3 * delta(|s| s.enemy_debuffs)
            + 2 * delta(|s| s.block).max(0)
            + 3 * delta(|s| s.pets)
            + HP_WEIGHT * delta(|s| s.hp)
    }
}
