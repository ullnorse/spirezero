//! A determinized combat search over blindfolded worlds.
//!
//! It samples `K` worlds from [`Combat::blindfold_for_search`] (the visible
//! position with the draw order and RNG streams resampled), a beam search over this turn's plays
//! in each, every plan scored at the start of the next player turn. An action's value is its
//! best plan's score averaged over the worlds; the bot takes the best and searches again at the
//! next decision, so a card drawn mid-turn is seen before the next choice.
//!
//! The world seeds are a hash of the fight's observation, so the search reads no hidden state:
//! the real draw order and streams are never played on. The plan scoring is RLB5's turn
//! evaluation (HP, enemy HP and kills, powers priced per kind).

use sts2_core::Combat;
use sts2_core::combat::{Action, CardId, CombatResult, CombatState, Decision, DecisionRequest};
use sts2_core::combat::{PromptText, Side, state::Creature};
use sts2_core::content::cards::CardType;
use sts2_core::content::monsters::MonsterKind;
use sts2_core::content::powers::{PowerKind, PowerType};
use sts2_rl::obs::{OBS_BYTES, Observer};

use super::ppo::Net;
use super::rollout::{RolloutNet, Scratch};
use super::run::card_base;
use super::value::ValueNet;

/// The most plays one turn's plan holds.
pub(super) const MAX_PLAYS: usize = 12;
/// The player's HP is worth this many points in a fight.
const HP_WEIGHT: i64 = 100;
/// Killing an enemy is worth this many points.
const KILL_WEIGHT: i64 = 1200;
/// A point of enemy debuff is worth this many points.
const DEBUFF_WEIGHT: i64 = 45;
/// A point of enemy buff (Strength from Enrage) costs this much: it scales every later hit.
const ENEMY_BUFF_WEIGHT: i64 = 120;
/// A point of player debuff costs this much.
const PLAYER_DEBUFF_WEIGHT: i64 = 60;
/// A pet's HP plus block is worth this many points.
const PET_WEIGHT: i64 = 4;
/// A point of a player buff is worth this many points.
const BUFF_WEIGHT: i64 = 4;
/// One potion in the plan costs this much: a resource, not free.
const POTION_COST: i64 = 15;
/// Playing a power is worth more than the turn shows: its effect runs for the rest of the fight.
pub(super) const POWER_BONUS: i64 = 120;
/// A point of `Stock` (a respawning enemy) is worth this much: reducing it is progress.
const STOCK_WEIGHT: i64 = 4000;
/// A point of `Sandpit` on an enemy buys a turn before it eats you.
const SANDPIT_WEIGHT: i64 = 300;
/// A point of `Infested` (a death that spawns four Wrigglers).
const INFESTED_WEIGHT: i64 = 4000;
/// A point of `Slumber` keeps the enemy asleep for a turn.
const SLUMBER_WEIGHT: i64 = 400;
/// A point of `Plow` is HP the Ceremonial Beast must lose before it is stunned.
const PLOW_WEIGHT: i64 = 50;
/// Each `CrabRage` on the Kaiser Crab's arms triggers on the other arm's death.
const CRAB_RAGE_WEIGHT: i64 = 2000;
/// Each `TestSubject` form killed is progress.
const RESPAWN_WEIGHT: i64 = 8000;
/// A won fight's score (finite, so `K` of them sum without overflow).
pub(super) const WON: i64 = 1_000_000_000_000;
/// A lost fight's score.
pub(super) const LOST: i64 = -WON;

/// How hard the search looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchConfig {
    /// Worlds sampled per decision.
    pub worlds: usize,
    /// Worlds sampled per decision in an elite or boss fight.
    pub big_worlds: usize,
    /// Worlds sampled per decision in a boss fight.
    pub boss_worlds: usize,
    /// Plays each candidate out to the fight's end in elite and boss fights.
    pub playouts: bool,
    /// The playouts use the combat net instead of the priority list.
    pub net_playouts: bool,
    /// Partial plans kept at each depth, per world.
    pub beam: usize,
    /// In elite and boss fights, this many of each first action's best turn ends are planned
    /// one more turn (in the same world, its next hand drawn) before the playout; 0 plans one.
    pub lookahead: usize,
    /// The later turns' beam width.
    pub beam2: usize,
    /// The turns planned after the first when `lookahead` is set (each later turn but the
    /// last keeps `lookahead` ends too).
    pub depth: usize,
    /// The fights the lookahead plans in.
    pub lookahead_in: Scope,
    /// With it non-zero, the lookahead runs only when the one-turn search's best first actions
    /// are within this many points a world of each other, and only among those.
    pub close: i64,
    /// Without the lookahead, a close decision is searched again in this many times the
    /// worlds (the first ones reused).
    pub close_worlds: usize,
    /// How the value net is used when the search has one ([`Playout::Value`]).
    pub value: ValueUse,
}

/// How the search uses the value net (RLB11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueUse {
    /// The turn ends are scored by the net instead of the turn score and a playout.
    pub leaves: bool,
    /// The fights the net scores the leaves in (elsewhere: the turn score and playouts).
    pub scope: Scope,
    /// Turn ends of each first action the net scores (the turn score's best ones).
    pub keep: usize,
    /// The HP fraction kept, in thousandths of a fight win.
    pub hp: i64,
    /// `hp` in the run's last fight (act 3's boss), where HP left over is worth nothing.
    pub last_boss_hp: Option<i64>,
    /// RLB18: `hp` in the act 1–2 boss fights, when set: at A0 the next act's Ancient heals all
    /// missing HP, so HP kept there is worth nothing.
    pub mid_boss_hp: Option<i64>,
    /// A potion's cost, in millionths of a fight win.
    pub potion: i64,
    /// RLB18: `potion` in boss fights, when set.
    pub boss_potion: Option<i64>,
    /// RLB16: a potion's cost outside boss fights from act `road_act` (0-based) on, when set: a
    /// potion drunk on the road is missing in the boss fight.
    pub road_potion: Option<i64>,
    /// The first act (0-based) [`ValueUse::road_potion`] holds in.
    pub road_act: usize,
    /// The policy head's prior at the root: millionths of a win per nat of log-probability.
    pub prior: i64,
    /// RLB14: thousandths of a leaf's value taken from a playout by the learned playout policy
    /// ([`Playout::Mixed`]) instead of the net; 0 scores by the net alone.
    pub mix: i64,
    /// The best ends (by the net) of each first action that are played out.
    pub mix_ends: usize,
    /// RLB15: the playouts of each end, averaged: the greedy one, then ones sampled from the
    /// policy.
    pub mix_samples: usize,
    /// The player turns a playout plays before the net scores where it stopped; 0 plays to the
    /// fight's end.
    pub mix_turns: usize,
    /// The fights the playouts are mixed in.
    pub mix_in: Scope,
}

impl ValueUse {
    /// The beam of a search with value leaves (RLB12; without them the beam stays 24).
    pub const BEAM: usize = 48;

    /// The playout's share of a boss leaf with value leaves, in thousandths (RLB14: 943
    /// against 923 of 1000 paired train runs, dev 925 against 907, test 187 against 180).
    pub const MIX: i64 = 300;

    /// The playouts by the net's policy; with `leaves` set, these settings.
    pub const DEFAULT: Self = Self {
        leaves: false,
        scope: Scope::All,
        // RLB12: 6 ends of each first action, a beam of 48 (`BEAM`): 618 against 592 of 663
        // paired train runs (keep 3, beam 24).
        keep: 6,
        hp: 500,
        last_boss_hp: None,
        mid_boss_hp: None,
        potion: 5_000,
        boss_potion: None,
        road_potion: None,
        road_act: 0,
        prior: 0,
        mix: 0,
        mix_ends: 1,
        mix_samples: 1,
        mix_turns: 0,
        mix_in: Scope::Boss,
    };
}

impl Default for ValueUse {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A set of fights by kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Boss fights.
    Boss,
    /// Elite and boss fights.
    Big,
    /// Every fight (common ones without playouts).
    All,
}

impl Scope {
    /// Whether a fight of `context` is in the set.
    #[must_use]
    pub const fn holds(self, context: FightContext) -> bool {
        match self {
            Self::Boss => context.boss,
            Self::Big => context.big,
            Self::All => true,
        }
    }
}

impl SearchConfig {
    /// The settings in a fight of `context`: [`ValueUse::road_potion`] outside boss fights,
    /// [`ValueUse::boss_potion`] in them, and [`ValueUse::mid_boss_hp`] in act 1–2 bosses.
    #[must_use]
    pub const fn in_fight(mut self, context: FightContext) -> Self {
        if let (true, Some(cost)) = (context.boss, self.value.boss_potion) {
            self.value.potion = cost;
        }
        if let (true, Some(hp)) = (context.boss && context.act < 2, self.value.mid_boss_hp) {
            self.value.hp = hp;
        }
        if let Some(cost) = self.value.road_potion
            && !context.boss
            && context.act >= self.value.road_act
        {
            self.value.potion = cost;
        }
        self
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        // RLB7: twice RLB6's worlds (8/8/32) won 249/1000 dev runs against 227.
        Self {
            worlds: 16,
            big_worlds: 16,
            boss_worlds: 64,
            playouts: true,
            net_playouts: false,
            beam: 24,
            lookahead: 0,
            beam2: 8,
            depth: 1,
            lookahead_in: Scope::Big,
            close: 0,
            close_worlds: 1,
            value: ValueUse::default(),
        }
    }
}

/// The fight's context the run shows: whether it is an elite or boss fight, and the act.
#[derive(Clone, Copy, Debug)]
pub struct FightContext {
    pub big: bool,
    pub boss: bool,
    pub act: usize,
}

/// What a fight's scores weigh. Measured on dev (RLB6): lowering the HP weight or the potion
/// cost in boss fights lost wins, because with a one-turn score HP is the survival signal; a
/// won fight already scores the same at any HP.
#[derive(Clone, Copy, Debug)]
pub(super) struct Weights {
    hp: i64,
    enemy_hp: i64,
    pub(super) potion: i64,
}

impl Weights {
    pub(super) const fn of(context: FightContext) -> Self {
        let enemy_hp = match context.act {
            0 => 50,
            2 => 30,
            _ => 40,
        };
        Self {
            hp: HP_WEIGHT,
            enemy_hp,
            potion: POTION_COST,
        }
    }
}

/// The search's action among `legal` (the fight's actions the run accepts, non-empty), its
/// playouts by the priority list or, with `config.net_playouts`, by `net`.
#[must_use]
pub fn choose(
    combat: &Combat,
    legal: &[Action],
    context: FightContext,
    config: SearchConfig,
    net: Option<&Net>,
) -> Action {
    let policy = match net {
        Some(net) if config.net_playouts => Playout::Net(net),
        _ => Playout::Priority,
    };
    choose_with(combat, legal, context, config, policy)
}

/// [`choose`] with the playouts by `policy`.
#[must_use]
pub fn choose_with(
    combat: &Combat,
    legal: &[Action],
    context: FightContext,
    config: SearchConfig,
    policy: Playout<'_>,
) -> Action {
    match scored_with(combat, legal, context, config, policy) {
        Searched::Forced(action) => action,
        Searched::Scored(scored) => scored.best().clone(),
    }
}

/// A searched decision: each first action's total over the worlds.
#[derive(Clone, Debug)]
pub struct Scored {
    pub first: Vec<Action>,
    pub totals: Vec<i64>,
    pub worlds: usize,
}

impl Scored {
    /// The first best in legal order: a stable tie-break.
    #[must_use]
    pub fn best(&self) -> &Action {
        let best =
            self.totals.iter().enumerate().fold(
                0,
                |best, (i, &t)| {
                    if t > self.totals[best] { i } else { best }
                },
            );
        &self.first[best]
    }
}

/// What [`scored_with`] found: a move with no alternative (or a choice's answer), or the
/// scores of the searched decision.
#[derive(Clone, Debug)]
pub enum Searched {
    Forced(Action),
    Scored(Scored),
}

/// The search of [`choose_with`], with each first action's total (RLB17: the expert-iteration
/// targets).
#[must_use]
pub fn scored_with(
    combat: &Combat,
    legal: &[Action],
    context: FightContext,
    config: SearchConfig,
    policy: Playout<'_>,
) -> Searched {
    scored_in_worlds(combat, legal, context, config, policy, None)
}

/// [`scored_with`] with each world's scores, one row a world in [`Scored::first`]'s order.
///
/// For the run review (WEB46: how many sampled worlds back a move), with a search without
/// [`SearchConfig::close`], whose worlds are all of one pass.
#[must_use]
pub fn scored_by_world(
    combat: &Combat,
    legal: &[Action],
    context: FightContext,
    config: SearchConfig,
    policy: Playout<'_>,
) -> (Searched, Vec<Vec<i64>>) {
    let mut rows = Vec::new();
    let searched = scored_in_worlds(combat, legal, context, config, policy, Some(&mut rows));
    (searched, rows)
}

#[allow(
    clippy::too_many_lines,
    reason = "one search, kept whole: the cases of a first action's scores read in order"
)]
fn scored_in_worlds(
    combat: &Combat,
    legal: &[Action],
    context: FightContext,
    config: SearchConfig,
    policy: Playout<'_>,
    mut rows: Option<&mut Vec<Vec<i64>>>,
) -> Searched {
    let config = config.in_fight(context);
    if combat.pending_decision().is_some()
        && let Some(answer) = answer_choice(combat)
    {
        return Searched::Forced(answer);
    }
    let first: Vec<Action> = legal
        .iter()
        .filter(|a| !matches!(a, Action::Resolve(_)))
        .cloned()
        .collect();
    if first.len() <= 1 {
        return Searched::Forced(first.into_iter().next().unwrap_or_else(|| legal[0].clone()));
    }
    let weights = Weights::of(context);
    // Playouts pay in elite and boss fights; in every fight they did not win more (dev, RLB6).
    let playout = (config.playouts && context.big).then_some(policy);
    let leaf_net = policy.leaves(config, context);
    let base = world_seed(combat);
    let worlds = if context.boss {
        config.boss_worlds
    } else if context.big {
        config.big_worlds
    } else {
        config.worlds
    };
    // The clairvoyant diagnostic searches the one true world (a ceiling, never the bot).
    let clairvoyant = CLAIRVOYANT.load(std::sync::atomic::Ordering::Relaxed);
    let worlds = if clairvoyant { 1 } else { worlds };
    let mut totals_of = |first: &[Action], lookahead: usize, ks: std::ops::Range<usize>| {
        let mut totals = vec![0_i64; first.len()];
        for k in ks {
            let world = if clairvoyant {
                combat.clone()
            } else {
                combat.blindfold_for_search(base ^ (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            };
            let scores = match (leaf_net, lookahead) {
                (Some(nets), _) => {
                    value_scores(&world, context.big, first, weights, config, lookahead, nets)
                }
                (None, 0) => {
                    world_scores(&world, context.big, first, weights, config.beam, playout)
                }
                (None, _) => two_turn_scores(
                    &world,
                    context.big,
                    first,
                    weights,
                    config,
                    lookahead,
                    playout,
                ),
            };
            if let Some(rows) = rows.as_deref_mut() {
                rows.push(scores.clone());
            }
            for (total, score) in totals.iter_mut().zip(scores) {
                *total = total.saturating_add(score);
            }
        }
        totals
    };
    let lookahead = if config.lookahead_in.holds(context) {
        config.lookahead
    } else {
        0
    };
    let (first, totals) = if config.close > 0 && (lookahead > 0 || config.close_worlds > 1) {
        // The one-turn search first; among the first actions within `close` points a world of
        // its best, when there are two, the lookahead, or more worlds.
        let totals = totals_of(&first, 0, 0..worlds);
        let top = totals.iter().copied().max().unwrap_or(LOST);
        let margin = config
            .close
            .saturating_mul(i64::try_from(worlds).unwrap_or(1));
        let (near, near_totals): (Vec<Action>, Vec<i64>) = first
            .iter()
            .zip(&totals)
            .filter(|&(_, &t)| t >= top.saturating_sub(margin))
            .map(|(a, &t)| (a.clone(), t))
            .unzip();
        if near.len() < 2 || top >= WON {
            (first, totals)
        } else if lookahead > 0 {
            let totals = totals_of(&near, lookahead, 0..worlds);
            (near, totals)
        } else {
            let more = totals_of(&near, 0, worlds..worlds * config.close_worlds);
            let totals = near_totals
                .iter()
                .zip(more)
                .map(|(a, b)| a.saturating_add(b))
                .collect();
            (near, totals)
        }
    } else {
        let totals = totals_of(&first, lookahead, 0..worlds);
        (first, totals)
    };
    let totals = with_prior(totals, leaf_net, combat, &first, config.value.prior, worlds);
    Searched::Scored(Scored {
        first,
        totals,
        worlds,
    })
}

/// `totals` (over `worlds` worlds) plus the policy head's prior (PUCT-style, one visit):
/// `prior` points a world per nat of log-probability; `totals` as they are without a net.
fn with_prior(
    totals: Vec<i64>,
    net: Option<(&ValueNet, Option<&RolloutNet>)>,
    combat: &Combat,
    first: &[Action],
    prior: i64,
    worlds: usize,
) -> Vec<i64> {
    let Some((net, _)) = net.filter(|_| prior > 0) else {
        return totals;
    };
    let per = prior.saturating_mul(i64::try_from(worlds).unwrap_or(1));
    totals
        .into_iter()
        .zip(net.log_prior(combat, first))
        .map(|(t, logp)| t.saturating_add(per.saturating_mul(logp) / 1000))
        .collect()
}

/// Whether the search looks at potions in this fight: in big fights, or under 60% HP. A potion's
/// first action outside that has no score, which reads as a lost fight but is not one.
#[must_use]
pub fn searches_potions(combat: &Combat, context: FightContext) -> bool {
    let (hp, max) = player_hp(combat.state());
    context.big || hp * 100 < max * 60
}

/// What the search saw at the end of a first action's best turn, over the sampled worlds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TurnEnd {
    /// The net's chance of winning the fight there, in thousandths (1000 once won, 0 once lost).
    pub win: u16,
    /// The player's HP there, the enemies' turn played, in tenths.
    pub hp_tenths: i32,
    /// Potions the turn drinks, summed over the worlds.
    pub potions: usize,
}

/// A [`TurnEnd`] for each of `picks` (indices into `first`, the search's own list).
///
/// In each world of [`scored_by_world`], the best end by the same value, read by the net. A boss
/// leaf's playout share is not in it: this is what the score is made of, not the score.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_arithmetic,
    reason = "a net's probability shown in thousandths and an average for a screen; no game rule"
)]
pub fn turn_ends(
    combat: &Combat,
    first: &[Action],
    context: FightContext,
    config: SearchConfig,
    net: &ValueNet,
    picks: &[usize],
) -> Vec<TurnEnd> {
    let config = config.in_fight(context);
    let weights = Weights::of(context);
    let base = world_seed(combat);
    let worlds = if context.boss {
        config.boss_worlds
    } else if context.big {
        config.big_worlds
    } else {
        config.worlds
    };
    let mut sums = vec![(0_f64, 0_i64, 0_usize); picks.len()];
    for k in 0..worlds {
        let world =
            combat.blindfold_for_search(base ^ (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let keep = config.value.keep.max(1);
        let tops = turn_tops(
            &world,
            context.big,
            first,
            weights,
            config.beam,
            keep,
            false,
        );
        for (sum, &pick) in sums.iter_mut().zip(picks) {
            let best = tops[pick]
                .iter()
                .filter_map(|t| {
                    let end = t.end.as_ref()?;
                    let potions = t
                        .plan
                        .iter()
                        .filter(|a| matches!(a, Action::UsePotion { .. }))
                        .count();
                    let cost = config.value.potion * i64::try_from(potions).unwrap_or(0);
                    let value = end_value(end, context.big, weights, config, 0, net) - cost;
                    Some((value, end, potions))
                })
                .max_by_key(|(value, ..)| *value);
            let Some((_, end, potions)) = best else {
                continue;
            };
            let end = resolve_choices(end.clone());
            let s = end.state();
            let hp = player_hp(s).0.max(0);
            sum.0 += if s.result == Some(CombatResult::Won) && hp > 0 {
                1.0
            } else if s.result.is_some() || hp <= 0 {
                0.0
            } else {
                f64::from(net.eval(&end).win)
            };
            sum.1 += i64::from(hp);
            sum.2 += potions;
        }
    }
    let n = worlds.max(1) as f64;
    sums.into_iter()
        .map(|(win, hp, potions)| TurnEnd {
            win: (win / n * 1000.0).round() as u16,
            hp_tenths: (hp as f64 * 10.0 / n).round() as i32,
            potions,
        })
        .collect()
}

/// A fight win on the value scale.
pub const VALUE_WIN: i64 = 1_000_000;

/// Each first action's value in `world` by the value net: the best over its
/// `config.value.keep` best turn ends (a beam of `beam`, by the turn score) of the end's value,
/// each end planned `turns` more turns in the same world first (by beams of `config.beam2`;
/// the next hands are the world's draws). One scale at every depth: [`VALUE_WIN`] × (P(win) +
/// w × the HP fraction kept), less the potions used.
///
/// With `mix` (RLB14) only the `config.value.mix_ends` best ends by that value count, each
/// scored again, [`ValueUse::mix`] of it by a playout of the learned policy from the end
/// ([`mixed_value`]).
fn value_scores(
    world: &Combat,
    big_fight: bool,
    first: &[Action],
    weights: Weights,
    config: SearchConfig,
    turns: usize,
    (net, mix): (&ValueNet, Option<&RolloutNet>),
) -> Vec<i64> {
    let opening = (config.beam, turns);
    let Some(rollout) = mix.filter(|_| turns == 0) else {
        return value_scores_in(world, big_fight, first, weights, config, opening, net);
    };
    let keep = config.value.keep.max(1);
    turn_tops(world, big_fight, first, weights, config.beam, keep, false)
        .into_iter()
        .map(|tops| {
            let mut scored: Vec<(i64, i64, Option<Combat>)> = tops
                .into_iter()
                .map(|t| {
                    let potions = t
                        .plan
                        .iter()
                        .filter(|a| matches!(a, Action::UsePotion { .. }))
                        .count();
                    let cost = config.value.potion * i64::try_from(potions).unwrap_or(0);
                    let end = t.end.map(resolve_choices);
                    let value = end.as_ref().map_or(LOST, |end| {
                        end_value(end, big_fight, weights, config, 0, net)
                    });
                    (value - cost, cost, end)
                })
                .collect();
            // Only the ends played out compete: an end not played out would win by having no
            // playout to lose.
            scored.sort_by_key(|(v, ..)| std::cmp::Reverse(*v));
            scored
                .iter()
                .take(config.value.mix_ends.max(1))
                .map(|(value, cost, end)| match end {
                    Some(end) if end.state().result.is_none() => {
                        mixed_value(end, *value + cost, config.value, net, rollout) - cost
                    }
                    _ => *value,
                })
                .max()
                .unwrap_or(LOST)
        })
        .collect()
}

/// `value` (the net's, of the turn's end `end`) mixed with a playout of `rollout` from `end`:
/// a won playout scores as a won fight, a lost one 0, and one stopped after
/// [`ValueUse::mix_turns`] turns by the net where it stopped. With
/// [`ValueUse::mix_samples`] above 1 the greedy playout is averaged with sampled ones.
fn mixed_value(
    end: &Combat,
    value: i64,
    config: ValueUse,
    net: &ValueNet,
    rollout: &RolloutNet,
) -> i64 {
    let samples = config.mix_samples.max(1);
    // The sampling stream is the search's own, fixed by the end's value: no hidden state.
    let mut rng = 0x5A3F_1E00_u64 ^ value.unsigned_abs();
    let played: i64 = (0..samples)
        .map(|k| {
            let stream = (k > 0).then_some(&mut rng);
            let last = play_out_with(
                end.clone(),
                Playout::Learned(rollout),
                config.mix_turns,
                stream,
            );
            let s = last.state();
            if s.result == Some(CombatResult::Won) && player_hp(s).0 > 0 {
                won_value(s, config)
            } else if s.result.is_some() || player_hp(s).0 <= 0 {
                0
            } else {
                net.score(&last, VALUE_WIN, config.hp)
            }
        })
        .sum::<i64>()
        / i64::try_from(samples).unwrap_or(1);
    (value * (1000 - config.mix) + played * config.mix) / 1000
}

/// [`value_scores`] by a beam of `beam` this turn.
fn value_scores_in(
    world: &Combat,
    big_fight: bool,
    first: &[Action],
    weights: Weights,
    config: SearchConfig,
    (beam, turns): (usize, usize),
    net: &ValueNet,
) -> Vec<i64> {
    let keep = config.value.keep.max(1);
    turn_tops(world, big_fight, first, weights, beam, keep, false)
        .into_iter()
        .map(|tops| {
            tops.into_iter()
                .map(|t| {
                    let potions = t
                        .plan
                        .iter()
                        .filter(|a| matches!(a, Action::UsePotion { .. }))
                        .count();
                    let value = t.end.as_ref().map_or(LOST, |end| {
                        end_value(end, big_fight, weights, config, turns, net)
                    });
                    value - config.value.potion * i64::try_from(potions).unwrap_or(0)
                })
                .max()
                .unwrap_or(LOST)
        })
        .collect()
}

/// A fight won with the player's HP in `s` on the value scale, with a certainty bonus over a
/// position the net gives the same value.
fn won_value(s: &CombatState, value: ValueUse) -> i64 {
    let (hp, max) = player_hp(s);
    VALUE_WIN
        + WON_BONUS
        + value.hp * (VALUE_WIN / 1000) * i64::from(hp.max(0)) / i64::from(max.max(1))
}

/// A won fight's edge over a position the net values the same (0.5% of a win).
const WON_BONUS: i64 = VALUE_WIN / 200;

/// The value of `end` (a turn's end) planned `turns` more turns: the net's at the last end.
fn end_value(
    end: &Combat,
    big_fight: bool,
    weights: Weights,
    config: SearchConfig,
    turns: usize,
    net: &ValueNet,
) -> i64 {
    let end = resolve_choices(end.clone());
    let s = end.state();
    if s.result == Some(CombatResult::Won) && player_hp(s).0 > 0 {
        return won_value(s, config.value);
    }
    if s.result.is_some() || player_hp(s).0 <= 0 {
        return 0;
    }
    if turns > 0 {
        let first: Vec<Action> = end
            .legal_actions()
            .into_iter()
            .filter(|a| !matches!(a, Action::Resolve(_)))
            .collect();
        if !first.is_empty() {
            let later = (config.beam2, turns - 1);
            return value_scores_in(&end, big_fight, &first, weights, config, later, net)
                .into_iter()
                .max()
                .unwrap_or(0)
                .max(0);
        }
    }
    net.score(&end, VALUE_WIN, config.value.hp)
}

/// A world seed from what the player sees: a hash of the fight's observation (and
/// [`SEARCH_SALT`]).
pub(super) fn world_seed(combat: &Combat) -> u64 {
    let mut out = vec![0; OBS_BYTES];
    Observer::new(true).observe(combat, &mut out);
    let salt = SEARCH_SALT.load(std::sync::atomic::Ordering::Relaxed);
    out.iter().fold(0xCBF2_9CE4_8422_2325_u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3)
    }) ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// The best score of a plan starting with each of `first` in `world`; [`LOST`] for one that
/// does not apply.
fn world_scores(
    world: &Combat,
    big_fight: bool,
    first: &[Action],
    weights: Weights,
    beam_width: usize,
    playout: Option<Playout<'_>>,
) -> Vec<i64> {
    world_plans(world, big_fight, first, weights, beam_width, playout).0
}

/// [`world_scores`] and the best plan (this turn's plays, the first action first) of each first
/// action.
pub(super) fn world_plans(
    world: &Combat,
    big_fight: bool,
    first: &[Action],
    weights: Weights,
    beam_width: usize,
    playout: Option<Playout<'_>>,
) -> (Vec<i64>, Vec<Vec<Action>>) {
    let tops = turn_tops(world, big_fight, first, weights, beam_width, 1, false);
    let mut scores = Vec::with_capacity(first.len());
    let mut plans = Vec::with_capacity(first.len());
    for (top, action) in tops.into_iter().zip(first) {
        let Some(best) = top.into_iter().next() else {
            scores.push(LOST);
            plans.push(vec![action.clone()]);
            continue;
        };
        let mut score = best.score;
        if let (Some(policy), Some(end)) = (playout, best.end)
            && score > LOST
            && score < WON
        {
            score += playout_value(end, policy);
        }
        scores.push(score);
        plans.push(best.plan);
    }
    (scores, plans)
}

/// One of a first action's best plans this turn: its score, the turn's end (for a fight won
/// this turn, the won state) and its plays.
pub(super) struct Top {
    pub(super) score: i64,
    pub(super) end: Option<Combat>,
    pub(super) plan: Vec<Action>,
}

/// Inserts `top` into `tops` (best first, at most `keep`): after equal scores, so the first
/// found of a score stays ahead; one whose score is already held is taken as the same end
/// reached in another order.
fn offer(tops: &mut Vec<Top>, top: Top, keep: usize) {
    if tops.iter().any(|t| t.score == top.score) && keep > 1 {
        return;
    }
    let at = tops.partition_point(|t| t.score >= top.score);
    if at < keep {
        tops.insert(at, top);
        tops.truncate(keep);
    }
}

/// A beam search over this turn's plays from `world`: the `keep` best plans of each first action,
/// each scored at the start of the next player turn.
fn turn_tops(
    world: &Combat,
    big_fight: bool,
    first: &[Action],
    weights: Weights,
    beam_width: usize,
    keep: usize,
    graded: bool,
) -> Vec<Vec<Top>> {
    // A later turn's win is worth a playout's win and the HP kept, not [`WON`]: a win found
    // there is one world's draw, and at [`WON`] the search traded any HP for it (RLB10).
    // A fight won in the enemy's turn (a poison) scores just under [`WON`] after a potion's
    // cost, so the won state, not the score, says so. A later turn's loss likewise is worth a
    // playout's loss, not [`LOST`], or the search only counted the worlds it dies in.
    let grade = |score: i64, end: &Combat| {
        if !graded {
            score
        } else if score >= WON || end.state().result == Some(CombatResult::Won) {
            let hp = i64::from(player_hp(end.state()).0);
            weights.hp * hp + PLAYOUT_WIN + 20 * hp
        } else if end.state().result.is_some() || player_hp(end.state()).0 <= 0 {
            lost_value(end, weights)
        } else {
            score
        }
    };
    let before = player_hp(world.state());
    let potions_ok = big_fight || before.0 * 100 < before.1 * 60;
    let mut tops: Vec<Vec<Top>> = first.iter().map(|_| Vec::new()).collect();
    // (state, index of its first action, a potion is in the plan, the plan)
    let mut beam: Vec<BeamEntry> = Vec::new();
    for (i, action) in first.iter().enumerate() {
        if matches!(action, Action::EndTurn) {
            let ended = end_turn_of(world.clone());
            let score = grade(eval(&ended, weights), &ended);
            if score > LOST {
                offer(
                    &mut tops[i],
                    Top {
                        score,
                        end: Some(ended),
                        plan: vec![action.clone()],
                    },
                    keep,
                );
            }
            continue;
        }
        if let Some((score, state, ended)) = expand(world, action, potions_ok, false, weights) {
            offer(
                &mut tops[i],
                Top {
                    score: grade(score, ended.as_ref().unwrap_or(&state)),
                    end: ended.or_else(|| (score >= WON).then(|| state.clone())),
                    plan: vec![action.clone()],
                },
                keep,
            );
            if score < WON {
                beam.push((
                    state,
                    i,
                    matches!(action, Action::UsePotion { .. }),
                    vec![action.clone()],
                ));
            }
        }
    }
    for _ in 1..MAX_PLAYS {
        let mut next: Vec<(i64, BeamEntry)> = Vec::new();
        for (state, root, potion_used, plan) in &beam {
            for action in state.legal_actions() {
                if matches!(action, Action::EndTurn | Action::Resolve(_)) {
                    continue;
                }
                let Some((score, copy, ended)) =
                    expand(state, &action, potions_ok, *potion_used, weights)
                else {
                    continue;
                };
                let mut plan = plan.clone();
                plan.push(action.clone());
                let worst = tops[*root]
                    .get(keep.saturating_sub(1))
                    .map_or(LOST, |t| t.score);
                let graded_score = grade(score, ended.as_ref().unwrap_or(&copy));
                if graded_score > worst || tops[*root].len() < keep {
                    offer(
                        &mut tops[*root],
                        Top {
                            score: graded_score,
                            end: ended.or_else(|| (score >= WON).then(|| copy.clone())),
                            plan: plan.clone(),
                        },
                        keep,
                    );
                }
                if score < WON {
                    let potion = *potion_used || matches!(action, Action::UsePotion { .. });
                    next.push((score, (copy, *root, potion, plan)));
                    // Keep at most twice the beam alive: a stable sort and truncation as it
                    // fills selects the same plans as one at the end, and a big hand's
                    // children are hundreds of fights held at once (RLB8).
                    if next.len() >= 2 * beam_width.max(1) {
                        keep_best(&mut next, beam_width);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        keep_best(&mut next, beam_width);
        beam = next.into_iter().map(|(_, entry)| entry).collect();
    }
    tops
}

/// Each first action's value planned `config.depth` more turns: its `lookahead` best turn
/// ends, each planned on by [`later_turns`] in the same world (the next hands are the world's
/// draws). The plans are chosen by their turn scores; the chosen line's last end is then played
/// out once, so a first action with more ends gains no edge from the max of noisy playouts.
/// The first turn's power bonus and potion cost are kept.
fn two_turn_scores(
    world: &Combat,
    big_fight: bool,
    first: &[Action],
    weights: Weights,
    config: SearchConfig,
    lookahead: usize,
    playout: Option<Playout<'_>>,
) -> Vec<i64> {
    let tops = turn_tops(
        world,
        big_fight,
        first,
        weights,
        config.beam,
        lookahead,
        false,
    );
    tops.into_iter()
        .map(|top| {
            let (score, end) =
                best_continuation(top, big_fight, weights, config, lookahead, config.depth);
            match (playout, end) {
                (Some(policy), Some(end)) if score > LOST && score < WON => {
                    score + playout_value(end, policy)
                }
                _ => score,
            }
        })
        .collect()
}

/// The best of `tops` (one first action's turn ends), each planned `turns` more turns: its
/// score and the last turn's end (`None` for a won or lost line).
fn best_continuation(
    tops: Vec<Top>,
    big_fight: bool,
    weights: Weights,
    config: SearchConfig,
    lookahead: usize,
    turns: usize,
) -> (i64, Option<Combat>) {
    tops.into_iter()
        .map(|t| match t.end {
            _ if t.score >= WON => (WON, None),
            None => (t.score, None),
            // A graded later-turn win or loss: the fight is over.
            Some(end) if end.state().result.is_some() || player_hp(end.state()).0 <= 0 => {
                (t.score, None)
            }
            Some(end) if turns == 0 => (t.score, Some(end)),
            Some(end) => {
                let carried = t.score - eval(&end, weights);
                later_turns(&end, big_fight, weights, config, lookahead, turns)
                    .map_or((LOST, None), |(v, end)| (v.saturating_add(carried), end))
            }
        })
        .max_by_key(|(score, _)| *score)
        .unwrap_or((LOST, None))
}

/// The best line from `start` (a player turn's start) planned `turns` turns by beams of
/// `config.beam2`: its score and its last end; `None` if every plan loses.
fn later_turns(
    start: &Combat,
    big_fight: bool,
    weights: Weights,
    config: SearchConfig,
    lookahead: usize,
    turns: usize,
) -> Option<(i64, Option<Combat>)> {
    // A choice the turn opens with is answered as the playouts answer it.
    let start = resolve_choices(start.clone());
    if start.state().result == Some(CombatResult::Lost) || player_hp(start.state()).0 <= 0 {
        return Some((lost_value(&start, weights), None));
    }
    if start.state().result.is_some() {
        return Some((eval(&start, weights), None));
    }
    let first: Vec<Action> = start
        .legal_actions()
        .into_iter()
        .filter(|a| !matches!(a, Action::Resolve(_)))
        .collect();
    if first.is_empty() {
        return Some((eval(&start, weights), Some(start)));
    }
    let keep = if turns > 1 { lookahead } else { 1 };
    let tops = turn_tops(&start, big_fight, &first, weights, config.beam2, keep, true);
    let mut tops: Vec<Top> = tops.into_iter().flatten().collect();
    if tops.is_empty() {
        return None;
    }
    // The `keep` best over every first action of this turn.
    tops.sort_by_key(|t| std::cmp::Reverse(t.score));
    tops.truncate(keep);
    Some(best_continuation(
        tops,
        big_fight,
        weights,
        config,
        lookahead,
        turns - 1,
    ))
}

/// A partial plan: its state, the index of its first action, whether a potion is in it, and
/// its actions.
type BeamEntry = (Combat, usize, bool, Vec<Action>);

/// The `width` best entries by score, ties in insertion order.
fn keep_best<T>(next: &mut Vec<(i64, T)>, width: usize) {
    next.sort_by_key(|(score, ..)| std::cmp::Reverse(*score));
    next.truncate(width);
}

/// How a playout picks its moves.
#[derive(Clone, Copy, Debug)]
pub enum Playout<'a> {
    /// [`priority_action`].
    Priority,
    /// The combat PPO net, greedy.
    Net(&'a Net),
    /// The learned playout policy ([`super::rollout`]), greedy.
    Learned(&'a RolloutNet),
    /// The value net's policy head ([`super::value`]), greedy; with
    /// [`ValueUse::leaves`] its value head scores the leaves instead.
    Value(&'a ValueNet),
    /// [`Playout::Value`], its leaves mixed with playouts of the learned policy
    /// ([`ValueUse::mix`]).
    Mixed(&'a ValueNet, &'a RolloutNet),
}

impl<'a> Playout<'a> {
    /// The value net that scores the leaves of a fight of `context`, if `config` asks for it.
    const fn leaf_net(self, config: SearchConfig, context: FightContext) -> Option<&'a ValueNet> {
        match self {
            Self::Value(net) | Self::Mixed(net, _)
                if config.value.leaves && config.value.scope.holds(context) =>
            {
                Some(net)
            }
            _ => None,
        }
    }

    /// [`Playout::leaf_net`] and the playout policy mixed into its leaves
    /// ([`Playout::mix_net`]).
    fn leaves(
        self,
        config: SearchConfig,
        context: FightContext,
    ) -> Option<(&'a ValueNet, Option<&'a RolloutNet>)> {
        self.leaf_net(config, context)
            .map(|net| (net, self.mix_net(config, context)))
    }

    /// The playout policy the leaves of a fight of `context` are mixed with, if any.
    const fn mix_net(self, config: SearchConfig, context: FightContext) -> Option<&'a RolloutNet> {
        match self {
            Self::Mixed(_, rollout)
                if config.value.mix > 0 && config.value.mix_in.holds(context) =>
            {
                Some(rollout)
            }
            _ => None,
        }
    }
}

/// The most actions a playout takes.
const PLAYOUT_ACTIONS: usize = 600;
/// A fight with more combat cards than this is running away on generated cards: a playout
/// stops there and the run ends as capped ([`super::terminal`]), since every search clone
/// grows with it (a memory bound, RLB8).
pub(super) const MAX_CARDS: usize = 2_000;
/// The most card plays a playout makes in one turn.
const PLAYOUT_PLAYS: usize = 25;

/// A playout's win, against 100 per point of HP in the turn score.
const PLAYOUT_WIN: i64 = 15_000;

/// The fight played to its end from `combat` by `policy`: a win is worth [`PLAYOUT_WIN`] plus a
/// little HP, a loss minus that, with up to half of it back for the enemy HP taken off.
#[must_use]
pub fn playout_value(combat: Combat, policy: Playout<'_>) -> i64 {
    let win = PLAYOUT_WIN;
    let start = enemy_total(combat.state()).max(1);
    let combat = play_out(combat, policy, 0);
    let s = combat.state();
    let hp = player_hp(s).0;
    if s.result == Some(CombatResult::Won) && hp > 0 {
        return win + 20 * i64::from(hp);
    }
    let left = enemy_total(s).clamp(0, start);
    -win + win * i64::from(start - left) / i64::from(start) / 2
}

/// `combat` played by `policy` to the fight's end or, with `turns` non-zero, until that many
/// player turns have ended.
#[must_use]
pub fn play_out(combat: Combat, policy: Playout<'_>, turns: usize) -> Combat {
    play_out_with(combat, policy, turns, None)
}

/// [`play_out`], a learned policy sampling its moves with `rng` if given.
fn play_out_with(
    mut combat: Combat,
    policy: Playout<'_>,
    turns: usize,
    mut rng: Option<&mut u64>,
) -> Combat {
    let mut ended = 0;
    let mut legal = Vec::new();
    let mut scratch = Scratch::default();
    let mut plays = 0;
    for _ in 0..PLAYOUT_ACTIONS {
        if combat.state().result.is_some() || combat.state().cards.len() > MAX_CARDS {
            break;
        }
        let action = if combat.pending_decision().is_some() {
            answer_choice(&combat)
        } else {
            combat.legal_actions_into(&mut legal);
            if plays >= PLAYOUT_PLAYS && legal.contains(&Action::EndTurn) {
                Some(Action::EndTurn)
            } else {
                match policy {
                    Playout::Priority => priority_action(&combat, &legal),
                    Playout::Net(net) => net.choose(&combat, &legal),
                    Playout::Learned(net) => net
                        .choose(&combat, &legal, &mut scratch, rng.as_deref_mut())
                        .or_else(|| priority_action(&combat, &legal)),
                    Playout::Value(net) | Playout::Mixed(net, _) => net
                        .choose(&combat, &legal)
                        .or_else(|| priority_action(&combat, &legal)),
                }
            }
        };
        let Some(action) = action.or_else(|| legal.first().cloned()) else {
            break;
        };
        let end = matches!(action, Action::EndTurn);
        plays = if end { 0 } else { plays + 1 };
        if combat.apply(action).is_err() {
            break;
        }
        ended += usize::from(end);
        if turns > 0 && ended >= turns {
            break;
        }
    }
    combat
}

/// A later turn's loss, on the playout's scale: a lost playout less the enemies left.
fn lost_value(end: &Combat, weights: Weights) -> i64 {
    let s = end.state();
    let alive = enemies_to_kill(s);
    -PLAYOUT_WIN
        - weights.enemy_hp * i64::from(enemy_hp_left(s))
        - KILL_WEIGHT * i64::try_from(alive).unwrap_or(0)
}

/// The living enemies' HP plus block.
fn enemy_total(s: &CombatState) -> i32 {
    living_enemies(s).map(standing_hp).sum()
}

fn living_enemies(s: &CombatState) -> impl Iterator<Item = &Creature> {
    s.enemies
        .iter()
        .map(|&id| s.creature(id))
        .filter(|c| c.is_alive())
}

/// The living enemies whose deaths end the fight: the primary ones. A secondary enemy (a
/// Minion: the Queen's amalgam, Kin followers, Fabricator bots) dies with the last primary one,
/// so its death is no step to the fight's end (the amalgam's enrages the Queen); its HP still
/// counts, as the threat it is. With none primary, or with [`MinionScoring::Kills`] (the
/// default), every living enemy.
fn enemies_to_kill(s: &CombatState) -> usize {
    let primary = living_enemies(s).filter(|c| c.is_primary_enemy()).count();
    if primary == 0 || minion_scoring() == MinionScoring::Kills {
        living_enemies(s).count()
    } else {
        primary
    }
}

/// RLB16 (`--minions kills|hp|primary`): how the turn score counts a Minion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinionScoring {
    /// Its HP and its kill, as every enemy's (the default).
    Kills = 0,
    /// Its HP, not its kill (the benchmark's 840 Queen fights won and lost exactly as `Kills`).
    Hp = 1,
    /// Neither, while a primary enemy lives (lost the benchmark: Queen 82.3% against 85.0%).
    Primary = 2,
}

/// The `--minions` setting, [`MinionScoring`] as a number.
pub static MINIONS: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn minion_scoring() -> MinionScoring {
    match MINIONS.load(std::sync::atomic::Ordering::Relaxed) {
        1 => MinionScoring::Hp,
        2 => MinionScoring::Primary,
        _ => MinionScoring::Kills,
    }
}

/// The HP a player still has to take off: [`enemy_total`], less every Minion's while a primary
/// enemy lives with [`MinionScoring::Primary`].
fn enemy_hp_left(s: &CombatState) -> i32 {
    let primary = living_enemies(s).any(Creature::is_primary_enemy);
    if primary && minion_scoring() == MinionScoring::Primary {
        living_enemies(s)
            .filter(|c| c.is_primary_enemy())
            .map(standing_hp)
            .sum()
    } else {
        enemy_total(s)
    }
}

/// The HP a player still has to take off `c`, block included. A Waterfall Giant killed with
/// Steam Eruption is back at 999,999,999 (max) HP, shown as infinite, only to explode and die
/// on its own: it counts 0, or the search scored the kill as the worst outcome and avoided it
/// (RLB9: 8 of 26 Giant fights won).
pub(super) const fn standing_hp(c: &Creature) -> i32 {
    if c.max_hp >= ABOUT_TO_BLOW_HP {
        0
    } else {
        c.hp + c.block
    }
}

/// A **clairvoyant** diagnostic (`bot --clairvoyant-fights`): the search sees the real draw
/// order and RNG. It measures a ceiling for the run policy; its results are never the bot's.
pub static CLAIRVOYANT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `--search-salt N` (RLB16): mixed into every world seed, so the same bot samples other
/// worlds. 0 (the default) is the bot as it was; another salt measures a run A/B's noise.
pub static SEARCH_SALT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// `WaterfallGiant`'s `TriggerAboutToBlowState` HP.
const ABOUT_TO_BLOW_HP: i32 = 999_999_999;

/// The playout's priority list: powers first, then the best card by its tier (attacks at the
/// weakest enemy), no potions, and the turn ends when nothing better is left.
fn priority_action(combat: &Combat, legal: &[Action]) -> Option<Action> {
    let s = combat.state();
    legal
        .iter()
        .filter_map(|a| {
            let Action::PlayCard { card, target } = a else {
                return None;
            };
            let kind = s.card(*card).kind;
            let class = match kind.card_type() {
                CardType::Power => 300,
                CardType::Attack | CardType::Skill => 100,
                _ => 0,
            };
            let target_hp = target.map_or(0, |t| s.creature(t).hp + s.creature(t).block);
            Some((
                class + 4 * card_base(kind) + 2 * i32::from(s.card(*card).upgrade_level),
                -target_hp,
                a,
            ))
        })
        .max_by_key(|(p, t, _)| (*p, *t))
        .map(|(.., a)| a.clone())
        .or_else(|| legal.iter().find(|a| matches!(a, Action::EndTurn)).cloned())
}

/// Plays `action` on a copy of `state` and scores the turn ended there; `None` if it does not
/// apply, is a potion the plan does not allow, or loses.
pub(super) fn expand(
    state: &Combat,
    action: &Action,
    potions_ok: bool,
    potion_used: bool,
    weights: Weights,
) -> Option<(i64, Combat, Option<Combat>)> {
    let potion = matches!(action, Action::UsePotion { .. });
    if potion && (!potions_ok || potion_used) {
        return None;
    }
    let power = matches!(action, Action::PlayCard { card, .. }
        if state.state().card(*card).kind.card_type() == CardType::Power);
    let mut copy = state.clone();
    copy.apply(action.clone()).ok()?;
    let copy = resolve_choices(copy);
    let s = copy.state();
    let alive = player_hp(s).0 > 0;
    if s.result == Some(CombatResult::Won) && alive {
        return Some((WON, copy, None));
    }
    if s.result == Some(CombatResult::Lost) || !alive {
        return None;
    }
    let ended = end_turn_of(copy.clone());
    let mut score = eval(&ended, weights);
    if power {
        let left: i32 = s
            .enemies
            .iter()
            .map(|&id| s.creature(id))
            .filter(|c| c.is_alive())
            .map(|c| c.hp + c.block)
            .sum();
        score += POWER_BONUS + 2 * i64::from(left);
    }
    if potion {
        score -= weights.potion;
    }
    Some((score, copy, Some(ended)))
}

/// The player's HP and max HP.
pub(super) fn player_hp(s: &CombatState) -> (i32, i32) {
    let p = s.creature(s.player.creature);
    (p.hp, p.max_hp)
}

/// The answer to a pending card choice: the best cards to keep, the worst to lose.
#[must_use]
pub fn answer_choice(combat: &Combat) -> Option<Action> {
    let request = combat.pending_decision()?;
    let DecisionRequest::ChooseCards {
        options,
        min,
        max,
        prompt,
        ..
    } = request;
    let mut ranked: Vec<(i64, usize)> = options
        .iter()
        .enumerate()
        .map(|(i, &card)| (card_value(combat, card), i))
        .collect();
    let low_first = matches!(
        prompt.text,
        PromptText::Discard | PromptText::Exhaust | PromptText::Remove | PromptText::Transform
    );
    ranked.sort_by_key(|(v, ..)| if low_first { *v } else { -*v });
    let take = if low_first { *min } else { *max };
    let cards: Vec<_> = ranked
        .iter()
        .take(take.clamp(*min, *max))
        .map(|&(_, i)| options[i])
        .collect();
    Some(Action::Resolve(Decision::Cards(cards)))
}

/// A crude value of a card for a choice that asks for one.
fn card_value(combat: &Combat, card: CardId) -> i64 {
    let c = combat.state().card(card);
    if c.kind.card_type() == CardType::Curse {
        return -100;
    }
    if matches!(c.kind.card_type(), CardType::Status | CardType::Quest) {
        return -50;
    }
    i64::from(card_base(c.kind)) + 2 * i64::from(c.upgrade_level)
}

/// Resolves any pending card choices on `combat`, choosing with [`answer_choice`].
pub(super) fn resolve_choices(mut combat: Combat) -> Combat {
    while combat.pending_decision().is_some() {
        let Some(answer) = answer_choice(&combat) else {
            break;
        };
        if combat.apply(answer).is_err() {
            break;
        }
    }
    combat
}

/// Finishes any pending card choice, then ends the turn, on `combat`.
pub(super) fn end_turn_of(combat: Combat) -> Combat {
    let mut combat = resolve_choices(combat);
    if combat.state().result.is_none() && combat.pending_decision().is_none() {
        let _ = combat.apply(Action::EndTurn);
    }
    combat
}

/// The sum of `kind`'s amounts on the living enemies.
fn enemy_power(enemies: &[&Creature], kind: PowerKind) -> i32 {
    enemies
        .iter()
        .flat_map(|c| &c.powers)
        .filter(|p| p.kind == kind)
        .map(|p| p.amount.abs())
        .sum()
}

/// The value of a position at the start of a player turn.
pub(super) fn eval(combat: &Combat, weights: Weights) -> i64 {
    let s = combat.state();
    if let Some(result) = s.result {
        return if result == CombatResult::Won {
            WON
        } else {
            LOST
        };
    }
    let player = s.creature(s.player.creature);
    let enemies: Vec<_> = s
        .enemies
        .iter()
        .map(|&id| s.creature(id))
        .filter(|c| c.is_alive())
        .collect();
    let sum_powers = |powers: &[sts2_core::combat::state::Power], t: PowerType| -> i32 {
        powers
            .iter()
            .filter(|p| p.kind.power_type() == t)
            .map(|p| p.amount.abs())
            .sum()
    };
    let barricade = player.powers.iter().any(|p| p.kind == PowerKind::Barricade);
    let standing = enemies_to_kill(s);
    let enemy_hp = enemy_hp_left(s);
    let enemy_debuffs: i32 = enemies
        .iter()
        .map(|c| sum_powers(&c.powers, PowerType::Debuff))
        .sum();
    let enemy_buffs: i32 = enemies
        .iter()
        .flat_map(|c| &c.powers)
        .filter(|p| {
            p.kind.power_type() == PowerType::Buff
                && !matches!(
                    p.kind,
                    PowerKind::Sandpit | PowerKind::Infested | PowerKind::Slumber
                )
        })
        .map(|p| p.amount.abs())
        .sum();
    let crab_rage = enemies
        .iter()
        .flat_map(|c| &c.powers)
        .filter(|p| p.kind == PowerKind::CrabRage)
        .count();
    let respawns: i32 = enemies
        .iter()
        .map(|c| {
            c.monster.as_ref().map_or(0, |m| match m.kind {
                MonsterKind::TestSubject(t) => i32::from(t.respawns),
                _ => 0,
            })
        })
        .sum();
    let pets: i32 = s
        .allies
        .iter()
        .map(|&id| s.creature(id))
        .filter(|c| c.side == Side::Player && c.pet_owner.is_some() && c.is_alive())
        .map(|c| c.hp + c.block)
        .sum();
    weights.hp * i64::from(player.hp)
        + if barricade {
            weights.hp * i64::from(player.block)
        } else {
            0
        }
        - weights.enemy_hp * i64::from(enemy_hp)
        - KILL_WEIGHT * i64::try_from(standing).unwrap_or(i64::MAX / 4)
        + DEBUFF_WEIGHT * i64::from(enemy_debuffs)
        - ENEMY_BUFF_WEIGHT * i64::from(enemy_buffs)
        - PLAYER_DEBUFF_WEIGHT * i64::from(sum_powers(&player.powers, PowerType::Debuff))
        - STOCK_WEIGHT * i64::from(enemy_power(&enemies, PowerKind::Stock))
        - INFESTED_WEIGHT * i64::from(enemy_power(&enemies, PowerKind::Infested))
        + SLUMBER_WEIGHT * i64::from(enemy_power(&enemies, PowerKind::Slumber))
        - PLOW_WEIGHT * i64::from(enemy_power(&enemies, PowerKind::Plow))
        - CRAB_RAGE_WEIGHT * i64::try_from(crab_rage).unwrap_or(0)
        + RESPAWN_WEIGHT * i64::from(respawns)
        + SANDPIT_WEIGHT * i64::from(enemy_power(&enemies, PowerKind::Sandpit))
        + PET_WEIGHT * i64::from(pets)
        + BUFF_WEIGHT * i64::from(sum_powers(&player.powers, PowerType::Buff))
}
