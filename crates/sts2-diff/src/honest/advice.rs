//! The bot as an adviser (the web client's "?", WEB10): its move and one line on why, both from
//! what the player sees.

#![allow(
    clippy::float_arithmetic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a net's probability shown in thousandths; it decides no game rule"
)]

use std::sync::Arc;

use sts2_core::Action;
use sts2_core::Combat;
use sts2_core::run::{RunAction, RunDecision, RunState};
use sts2_rl::run_action::run_level_index;

use super::policy::RunNet;
use super::rollout::RolloutNet;
use super::search::{self, Playout, Scope, SearchConfig, Searched, TurnEnd, VALUE_WIN, ValueUse};
use super::value::ValueNet;
use super::{Bot, Controller, context_of};

/// Why the bot chose what it did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// In a fight: the value net's chance of winning it, in thousandths.
    Win(u16),
    /// A run choice by the learned run policy: its probability of that choice, in thousandths.
    Sure(u16),
    /// A run choice by the hand rules (no run policy, or a rule over it).
    Rules,
}

impl Why {
    /// The reason as a short line.
    #[must_use]
    pub fn line(self) -> String {
        match self {
            Self::Win(p) if p >= 995 => "Win chance 99%+".to_owned(),
            Self::Win(p) if p < 5 => "Win chance under 1%".to_owned(),
            Self::Win(p) => format!("Win chance {}%", (p + 5) / 10),
            Self::Sure(p) => format!("{}% sure", (p + 5) / 10),
            Self::Rules => "By the bot's rules".to_owned(),
        }
    }
}

/// Thousandths of a probability.
fn permille(p: f32) -> u16 {
    (p.clamp(0.0, 1.0) * 1000.0).round() as u16
}

/// The champion's search at `worlds`, `big_worlds` and `boss_worlds` sampled worlds a decision.
///
/// As `sts2-diff bot --run-policy builtin --value builtin --value-leaves --value-hp 300`:
/// RLB12's value leaves and RLB14's playout blend in boss fights.
#[must_use]
pub const fn champion_search(worlds: usize, big_worlds: usize, boss_worlds: usize) -> SearchConfig {
    SearchConfig {
        worlds,
        big_worlds,
        boss_worlds,
        playouts: true,
        net_playouts: false,
        beam: ValueUse::BEAM,
        lookahead: 0,
        beam2: 8,
        depth: 1,
        lookahead_in: Scope::Big,
        close: 0,
        close_worlds: 1,
        value: ValueUse {
            leaves: true,
            hp: 300,
            mix: ValueUse::MIX,
            ..ValueUse::DEFAULT
        },
    }
}

/// The phone's search: the champion's at a quarter of its worlds (16/16/64), so a decision
/// takes about a second in a phone's browser.
pub const PHONE: SearchConfig = champion_search(4, 4, 12);

impl Bot {
    /// The champion bot (see [`champion_search`]) with its fights searched by `search`.
    ///
    /// # Panics
    /// If the committed value net does not load (it is checked by `the_builtin_net_matches_pytorch`).
    #[must_use]
    pub fn champion(search: SearchConfig) -> Self {
        let value = ValueNet::builtin().expect("the committed value net loads");
        Self {
            controller: Controller::SearchValue(
                search,
                Arc::new(value),
                Some(Arc::new(RolloutNet::v2())),
            ),
            net: None,
            run_policy: Some(Arc::new(RunNet::builtin())),
        }
    }

    /// [`Bot::choose`] and why: in a fight the value net's win chance of the position, else
    /// the run policy's probability of its choice. Reads only what the player sees.
    #[must_use]
    pub fn advise(
        &self,
        s: &RunState,
        decision: &RunDecision<'_>,
        legal: &[RunAction],
    ) -> (RunAction, Why) {
        let action = self.choose(s, decision, legal);
        let why = match decision {
            RunDecision::Combat(combat) => match &self.controller {
                Controller::SearchValue(_, net, _) | Controller::Az(_, _, net) => {
                    Why::Win(permille(net.eval(combat).win))
                }
                _ => Why::Rules,
            },
            _ => self
                .run_policy
                .as_ref()
                .and_then(|net| net.choose_sure(s, decision, legal))
                .filter(|(a, _)| *a == action)
                .map_or(Why::Rules, |(_, p)| Why::Sure(permille(p))),
        };
        (action, why)
    }
}

/// A fight move the bot scores below its own (the run review, WEB46).
#[derive(Clone, Debug)]
pub struct FightGap {
    /// The bot's move.
    pub best: Action,
    /// The player's move and the bot's, each its average over the worlds in millionths of a
    /// fight win: P(win) + the HP weight × the HP share kept, less the potions used.
    pub mine: i64,
    pub theirs: i64,
    /// The sampled worlds where the bot's move scores above, and below, the player's.
    pub ahead: usize,
    pub behind: usize,
    pub worlds: usize,
    /// The moves the search compared.
    pub options: usize,
}

/// A run choice the run policy prefers another to.
#[derive(Clone, Debug)]
pub struct ChoiceGap {
    /// The bot's choice.
    pub best: RunAction,
    /// The policy's probability of the player's choice and of its own, in thousandths.
    pub mine: u16,
    pub theirs: u16,
    /// The choices the policy compared.
    pub options: usize,
}

impl Bot {
    /// How the bot's search scores `mine` against its own move at this fight decision: `None`
    /// when it has no scores here (a forced move, a card choice's answer, a potion it does not
    /// look at, or `mine` is not among the moves it searched) or does not rate its own move higher.
    #[must_use]
    pub fn fight_gap(
        &self,
        s: &RunState,
        combat: &Combat,
        legal: &[RunAction],
        mine: &Action,
    ) -> Option<FightGap> {
        let Controller::SearchValue(config, net, rollout) = &self.controller else {
            return None;
        };
        if matches!(mine, Action::UsePotion { .. })
            && !search::searches_potions(combat, context_of(s))
        {
            return None;
        }
        let fight: Vec<Action> = legal
            .iter()
            .filter_map(|a| match a {
                RunAction::Combat(a) => Some(a.clone()),
                _ => None,
            })
            .collect();
        let policy = rollout
            .as_deref()
            .map_or(Playout::Value(net), |r| Playout::Mixed(net, r));
        let (searched, rows) =
            search::scored_by_world(combat, &fight, context_of(s), *config, policy);
        let Searched::Scored(scored) = searched else {
            return None;
        };
        let best = scored.best().clone();
        let at = |a: &Action| scored.first.iter().position(|x| x == a);
        let (me, bot) = (at(mine)?, at(&best)?);
        // A world with no end for a move (it cannot be played out) reads as a lost fight.
        let read = |x: i64| x.max(0);
        let n = i64::try_from(rows.len().max(1)).ok()?;
        let mean = |i: usize| rows.iter().map(|r| read(r[i])).sum::<i64>() / n;
        let (mine, theirs) = (mean(me), mean(bot));
        (theirs > mine).then(|| FightGap {
            best,
            mine,
            theirs,
            ahead: rows.iter().filter(|r| read(r[bot]) > read(r[me])).count(),
            behind: rows.iter().filter(|r| read(r[bot]) < read(r[me])).count(),
            worlds: rows.len(),
            options: scored.first.len(),
        })
    }

    /// What the search saw at the end of the turn for the moves `picks`: see [`search::turn_ends`].
    #[must_use]
    pub fn turn_ends(
        &self,
        s: &RunState,
        combat: &Combat,
        legal: &[RunAction],
        picks: &[&Action],
    ) -> Option<Vec<TurnEnd>> {
        let Controller::SearchValue(config, net, _) = &self.controller else {
            return None;
        };
        let first: Vec<Action> = legal
            .iter()
            .filter_map(|a| match a {
                RunAction::Combat(a) if !matches!(a, Action::Resolve(_)) => Some(a.clone()),
                _ => None,
            })
            .collect();
        let picks = picks
            .iter()
            .map(|p| first.iter().position(|a| a == *p))
            .collect::<Option<Vec<_>>>()?;
        Some(search::turn_ends(
            combat,
            &first,
            context_of(s),
            *config,
            net,
            &picks,
        ))
    }

    /// How the run policy rates `mine` against its own choice: `None` when the choice is not the
    /// policy's (a hand rule's), it does not list `mine`, or both are the same choice.
    #[must_use]
    pub fn choice_gap(
        &self,
        s: &RunState,
        decision: &RunDecision<'_>,
        legal: &[RunAction],
        mine: &RunAction,
    ) -> Option<ChoiceGap> {
        let net = self.run_policy.as_ref()?;
        let shares = net.shares(s, decision, legal);
        let best = self.choose(s, decision, legal);
        let (top_index, _, top) = shares
            .iter()
            .copied()
            .find(|&(_, position, _)| legal.get(position) == Some(&best))?;
        let index = run_level_index(mine)?;
        let (_, _, share) = shares.iter().copied().find(|&(i, ..)| i == index)?;
        (index != top_index).then(|| ChoiceGap {
            best,
            mine: permille(share),
            theirs: permille(top),
            options: shares.len(),
        })
    }
}

/// A score of [`FightGap`] as thousandths of a fight win.
#[must_use]
pub const fn score_permille(score: i64) -> i64 {
    score / (VALUE_WIN / 1000)
}
