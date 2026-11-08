//! The greedy play policy: a simple heuristic that survives much longer than random play, so
//! the run comparison reaches the later floors and the boss.
//!
//! In combat it is `sts2_rl::resolve::Greedy`: each turn is planned with a small beam search over
//! play sequences, each scored by ending the turn on a copy (damage dealt, enemies killed,
//! debuffs, HP lost, Osty's health); potions only when low (or in elite and boss fights when
//! hurt), card choices take the first options. Out of combat it heads for rest sites and away
//! from elites when hurt, takes the rarest card reward, judges events and Neow by their immediate outcome, and otherwise takes
//! the first choice. It only ever picks from the actions it is given (the ones the engine
//! models).

use std::collections::HashMap;

use sts2_core::Combat;
use sts2_core::combat::Action;
use sts2_core::content::ancients::AncientKind;
use sts2_core::content::cards::{CardRarity, CardType};
use sts2_core::run::{
    MapCoord, MapPointType, RestOptionKind, Reward, Run, RunAction, RunCard, RunDecision,
};
use sts2_rl::resolve::Greedy as CombatGreedy;

/// Rest instead of upgrading below this share of max HP (in percent).
const REST_HP_PERCENT: i32 = 60;
/// Rest instead of upgrading before the boss below this share of max HP (in percent).
const BOSS_REST_HP_PERCENT: i32 = 90;
/// Neow's curse option is taken only if it is clearly better.
const CURSE_PENALTY: i64 = 30;
/// Elites are avoided below this share of max HP (in percent).
const ELITE_HP_PERCENT: i32 = 80;

/// The greedy policy: its combat half is `sts2_rl::resolve::Greedy` (the run env's resolver),
/// which keeps the rest of the turn it planned.
#[derive(Clone, Debug, Default)]
pub struct Greedy {
    combat: CombatGreedy,
}

impl Greedy {
    /// The greedy choice among `modeled` (non-empty), the actions the engine models right now.
    /// A card choice inside combat is answered with [`Greedy::answer`] instead.
    pub fn choose(&mut self, run: &Run, modeled: &[RunAction]) -> RunAction {
        if let RunDecision::Combat(combat) = run.pending_decision() {
            return self.combat.choose(run, combat, modeled);
        }
        self.combat.clear();
        choose_out_of_combat(run, modeled)
    }

    /// The answer to a card choice inside combat: the first options, as the plan assumed.
    #[must_use]
    pub fn answer(combat: &Combat) -> Option<Action> {
        CombatGreedy::answer(combat)
    }
}

/// [`Greedy::choose`] outside combat.
fn choose_out_of_combat(run: &Run, modeled: &[RunAction]) -> RunAction {
    let pick = match run.pending_decision() {
        RunDecision::Map(_) => map_node(run, modeled),
        RunDecision::RestSite(options) => {
            let s = run.state();
            // Before the boss, rest unless nearly full.
            let before_boss = s
                .coord()
                .is_some_and(|c| c.row + 1 == s.map.boss().coord.row);
            let threshold = if before_boss {
                BOSS_REST_HP_PERCENT
            } else {
                REST_HP_PERCENT
            };
            let hurt = s.hp * 100 < s.max_hp * threshold;
            // Hatching turns a dead egg into a free 14-damage attack, so it comes before Smith.
            let hatch = options.iter().any(|o| o.kind == RestOptionKind::Hatch);
            let want = if hurt {
                RestOptionKind::Heal
            } else if hatch {
                RestOptionKind::Hatch
            } else {
                RestOptionKind::Smith
            };
            options
                .iter()
                .position(|o| o.kind == want)
                .map(RunAction::RestOption)
                .filter(|a| modeled.contains(a))
        }
        // The best card of the first card reward; gold and the like are claimed by default.
        RunDecision::Rewards(rewards) => rewards.iter().enumerate().find_map(|(i, r)| match r {
            Reward::Card(cards) => card_pick(&cards.cards, modeled, i),
            _ => None,
        }),
        // Neow's last option is its curse.
        RunDecision::Ancient(room) if room.kind == AncientKind::Neow => {
            let curse = room.options.len() - 1;
            best_by_outcome(run, modeled, |a| match a {
                RunAction::AncientOption(i) if *i == curse => -CURSE_PENALTY,
                _ => 0,
            })
        }
        RunDecision::Ancient(_) | RunDecision::Event(_) => best_by_outcome(run, modeled, |_| 0),
        // Shopping is left to the random policy; the greedy one walks out.
        RunDecision::Shop(_) => Some(RunAction::Proceed).filter(|a| modeled.contains(a)),
        _ => None,
    };
    pick.unwrap_or_else(|| {
        // Never a potion out of combat.
        modeled
            .iter()
            .find(|a| !matches!(a, RunAction::UsePotion(_)))
            .unwrap_or(&modeled[0])
            .clone()
    })
}

/// The choice whose immediate outcome on a copy of the run is best: HP, max HP (weighed more)
/// and a little gold, plus `bias`. Potions are not considered.
fn best_by_outcome(
    run: &Run,
    modeled: &[RunAction],
    bias: impl Fn(&RunAction) -> i64,
) -> Option<RunAction> {
    let value = |r: &Run| {
        let s = r.state();
        let alive = if r.pending_decision() == RunDecision::GameOver {
            -1000
        } else {
            0
        };
        alive + 3 * i64::from(s.max_hp) + i64::from(s.hp) + i64::from(s.gold) / 10
    };
    let now = value(run);
    modeled
        .iter()
        .filter(|a| !matches!(a, RunAction::UsePotion(_)))
        .filter_map(|a| {
            let mut copy = run.clone();
            copy.apply(a.clone()).ok()?;
            Some((value(&copy) - now + bias(a), a))
        })
        // The first of equals.
        .rev()
        .max_by_key(|(score, _)| *score)
        .map(|(_, a)| a.clone())
}

/// The map: the child on the best path to the boss (a path's worth is the sum of its points':
/// rooms that give rewards or rest count for it, elites against it), a rest site right away
/// when hurt, an elite only when healthy.
fn map_node(run: &Run, modeled: &[RunAction]) -> Option<RunAction> {
    let s = run.state();
    let hurt = s.hp * 100 < s.max_hp * REST_HP_PERCENT;
    let healthy = s.hp * 100 >= s.max_hp * ELITE_HP_PERCENT;
    let worth = |kind: MapPointType| -> i32 {
        match kind {
            MapPointType::RestSite => 2,
            MapPointType::Monster | MapPointType::Unknown | MapPointType::Treasure => 1,
            MapPointType::Elite => -4,
            _ => 0,
        }
    };
    // Best path worth from each point, by row from the top down.
    let mut best: HashMap<MapCoord, i32> = HashMap::new();
    for p in s.map.points().iter().rev() {
        let rest = p
            .children
            .iter()
            .filter_map(|c| best.get(&c))
            .max()
            .copied()
            .unwrap_or(0);
        best.insert(p.coord, worth(p.kind) + rest);
    }
    modeled
        .iter()
        .filter_map(|a| match a {
            RunAction::ChooseNode(c) => s.map.point(*c).map(|p| {
                let now = match p.kind {
                    MapPointType::RestSite if hurt => 100,
                    MapPointType::Elite if healthy => 4,
                    _ => 0,
                };
                (now + best.get(c).copied().unwrap_or(0), a)
            }),
            _ => None,
        })
        .rev()
        .max_by_key(|(worth, _)| *worth)
        .map(|(_, a)| a.clone())
}

/// A card reward pick: the rarest card, attacks first among equals.
fn card_pick(cards: &[RunCard], modeled: &[RunAction], reward: usize) -> Option<RunAction> {
    modeled
        .iter()
        .filter_map(|a| match a {
            RunAction::PickCard { reward: r, card } if *r == reward => {
                let kind = cards[*card].to_combat()?.kind;
                let rarity = match kind.rarity() {
                    CardRarity::Rare => 3,
                    CardRarity::Uncommon => 2,
                    _ => 1,
                };
                let attack = i32::from(kind.card_type() == CardType::Attack);
                Some((2 * rarity + attack, a))
            }
            _ => None,
        })
        .rev()
        .max_by_key(|(worth, _)| *worth)
        .map(|(_, a)| a.clone())
}
