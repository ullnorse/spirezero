//! Combat resolvers for the run env's hybrid mode (`docs/reviews/spire-mentor.md`, idea 1): the
//! agent makes the run-level choices and a [`Resolver`] plays each fight inside the env.
//!
//! [`Greedy`] is the sweep's greedy combat policy (`cargo xtask sweep --policy greedy`, which
//! `sts2-diff` drives through this module): each turn is planned by a small beam search over play
//! sequences, each scored by ending the turn on a copy (damage dealt, enemies killed, debuffs, HP
//! lost, Osty's health); potions only when low (or in elite and boss fights when hurt), card
//! choices take the first options. [`Random`] plays uniformly random legal actions. Both only
//! ever pick from the legal list they are given, and both are deterministic given the run's seed.

use std::collections::VecDeque;
use std::fmt;

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
const MAX_PLAYS: usize = 10;
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
    let before = Snapshot::of(combat.state());
    let potions_ok = before.hp * 100 < before.max_hp * POTION_HP_PERCENT
        || (big_fight && before.hp * 100 < before.max_hp * BIG_FIGHT_POTION_HP_PERCENT);
    let mut best = (
        before.score_to(&Snapshot::of(end_turn_of(combat.clone()).state())),
        Vec::new(),
    );
    let mut beam: Vec<(Combat, Vec<Action>, i64)> = vec![(combat.clone(), Vec::new(), 0)];
    for depth in 0..MAX_PLAYS {
        let mut next: Vec<(i64, Combat, Vec<Action>, i64)> = Vec::new();
        for (state, plan, bonus) in &beam {
            // The first play must be one the run models; later ones are checked when made.
            let legal = if depth == 0 {
                first.to_vec()
            } else {
                state.legal_actions()
            };
            for a in legal {
                let potion = matches!(a, Action::UsePotion { .. });
                if matches!(a, Action::EndTurn | Action::Resolve(_)) || (potion && !potions_ok) {
                    continue;
                }
                let power = matches!(a, Action::PlayCard { card, .. }
                    if state.state().card(card).card_type() == CardType::Power);
                let mut copy = state.clone();
                if copy.apply(a.clone()).is_err() {
                    continue;
                }
                let copy = resolve_choices(copy);
                let mut plan = plan.clone();
                plan.push(a);
                if copy.state().result.is_some() && Snapshot::of(copy.state()).hp > 0 {
                    return plan;
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
                let score = before
                    .score_to(&Snapshot::of(end_turn_of(copy.clone()).state()))
                    .saturating_add(bonus);
                next.push((score, copy, plan, bonus));
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort_by_key(|(score, ..)| std::cmp::Reverse(*score));
        next.truncate(BEAM);
        if next[0].0 > best.0 {
            best = (next[0].0, next[0].2.clone());
        }
        beam = next
            .into_iter()
            .map(|(_, state, plan, bonus)| (state, plan, bonus))
            .collect();
    }
    best.1
}

/// Answers any pending card choice with the first options.
fn resolve_choices(mut combat: Combat) -> Combat {
    while combat.pending_decision().is_some() {
        let Some(answer) = whole_choice(&combat, |_| 0) else {
            break;
        };
        if combat.apply(answer).is_err() {
            break;
        }
    }
    combat
}

/// Finishes any pending card choice with the first options, then ends the turn, on `combat`.
fn end_turn_of(combat: Combat) -> Combat {
    let mut combat = resolve_choices(combat);
    if combat.state().result.is_none() && combat.pending_decision().is_none() {
        let _ = combat.apply(Action::EndTurn);
    }
    combat
}

/// What the lookahead compares.
struct Snapshot {
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
    fn of(s: &CombatState) -> Self {
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
    fn score_to(&self, after: &Self) -> i64 {
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
