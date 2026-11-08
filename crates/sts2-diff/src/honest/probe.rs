//! The deck probe (RLB18): a run's current deck, relics, belt and HP played against the act's
//! boss by the learned playout policy, in many fresh worlds.
//!
//! The boss is the one the map shows; the worlds come from fixed seeds (the same for every
//! candidate, so two decks are compared on common random numbers), not from the run's streams,
//! so nothing hidden is read.

use sts2_core::combat::{CombatResult, Encounter};
use sts2_core::run::{
    Reward, RoomType, RunAction, RunDecision, RunRelic, RunState, SelectPurpose, potions, relics,
    shop,
};
use sts2_core::{Combat, CombatConfig};

use super::policy::{Features, RunNet};
use super::rollout::RolloutNet;
use super::search::{self, Playout};

/// The probe's result: fights won of `n`, and the HP left summed over the won ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Probe {
    pub won: u32,
    pub n: u32,
    pub hp_left: i64,
}

impl Probe {
    /// The share of fights won, in millionths.
    #[must_use]
    pub fn win_ppm(self) -> i64 {
        i64::from(self.won) * 1_000_000 / i64::from(self.n.max(1))
    }
}

/// The act's (next) boss fight from `s`, in the world of `world`: the run's deck, relics, belt,
/// HP and energy, on fresh streams. `None` if the run holds something the fight cannot set up.
#[must_use]
pub fn boss_combat(s: &RunState, world: u64) -> Option<Combat> {
    let info = s.act().next_boss_encounter();
    let encounter = Encounter::from_entry(info.id)?;
    let deck = s
        .deck
        .iter()
        .map(|c| c.to_combat())
        .collect::<Option<Vec<_>>>()?;
    let mut config = CombatConfig::new(s.character, &format!("PROBE{world}"), s.ascension).ok()?;
    let belt = s
        .potions
        .iter()
        .map(|p| p.and_then(potions::combat_potion).map(Into::into))
        .collect();
    let to_i32 = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
    let boss = s.map.boss().coord;
    config.encounter = encounter;
    config.room_type = RoomType::Boss;
    config.deck = deck;
    config.relics = relics::combat_relics(&s.relics);
    config.potion_slots = s.potions.len();
    config.potions = belt;
    config.player_hp = s.hp;
    config.player_max_hp = s.max_hp;
    config.max_energy = s.max_energy;
    config.gold = s.gold;
    config.total_floor = to_i32(s.total_floor());
    config.act_index = to_i32(s.act_index);
    config.map_coord = Some((to_i32(boss.col), to_i32(boss.row)));
    Some(Combat::new(config))
}

/// `worlds` playouts by `net` of the act's boss from `s` (worlds `0..worlds`).
#[must_use]
pub fn probe(s: &RunState, worlds: u64, net: &RolloutNet) -> Probe {
    let mut out = Probe::default();
    for world in 0..worlds {
        let Some(combat) = boss_combat(s, world) else {
            return out;
        };
        let end = search::play_out(combat, Playout::Learned(net), 0);
        let state = end.state();
        let hp = state.creature(state.player.creature).hp;
        out.n += 1;
        if state.result == Some(CombatResult::Won) && hp > 0 {
            out.won += 1;
            out.hp_left += i64::from(hp);
        }
    }
    out
}

/// How the run decisions use the probe (`bot --deck-probe W,M,K`).
#[derive(Clone, Copy, Debug)]
pub struct ProbeUse {
    /// Playouts per candidate.
    pub worlds: u64,
    /// The run net's top two candidates are probed when their logits are this close.
    pub margin: f32,
    /// Logit added per unit of probed P(win).
    pub weight: f32,
}

/// The probe the run decisions use, with its playout net; unset leaves them to the run net.
pub static PROBE: std::sync::OnceLock<(ProbeUse, RolloutNet)> = std::sync::OnceLock::new();

/// Close decisions the probe scored, and those where it changed the run net's choice.
pub static PROBED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// See [`PROBED`].
pub static FLIPPED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The run state `action` leads to, when its effect is plain from what the player sees.
///
/// That is a card picked or skipped, a card upgraded or removed by a selection,
/// a shop card, relic or potion bought, or leaving. `None` otherwise.
#[must_use]
pub fn after(s: &RunState, decision: &RunDecision<'_>, action: &RunAction) -> Option<RunState> {
    let mut next = s.clone();
    match (decision, action) {
        (RunDecision::Rewards(_), RunAction::SkipReward(_) | RunAction::Proceed)
        | (RunDecision::Shop(_), RunAction::Proceed) => {}
        (RunDecision::Rewards(rewards), &RunAction::PickCard { reward, card }) => {
            let Reward::Card(cards) = rewards.get(reward)? else {
                return None;
            };
            next.deck.push(*cards.cards.get(card)?);
        }
        (RunDecision::CardSelect(select), &RunAction::SelectCard(option)) => {
            if !select.offered.is_empty() {
                return None;
            }
            let index = *select.options.get(option)?;
            match select.purpose {
                SelectPurpose::Upgrade | SelectPurpose::Smith => {
                    let card = next.deck.get_mut(index)?;
                    if !card.is_upgradable() {
                        return None;
                    }
                    card.upgrade();
                }
                SelectPurpose::Remove { .. } | SelectPurpose::ShopRemoval { .. } => {
                    next.deck.remove(index);
                }
                _ => return None,
            }
        }
        (RunDecision::Shop(shop), &RunAction::BuyCard(i)) => {
            let entry = shop.cards.get(i)?;
            next.deck.push(entry.card?);
            next.gold -= shop::price(s, entry.base_cost);
        }
        (RunDecision::Shop(shop), &RunAction::BuyRelic(i)) => {
            let entry = shop.relics.get(i)?;
            next.relics
                .push(RunRelic::new(crate::ids::relic_from_id(entry.id?)?));
            next.gold -= shop::price(s, entry.base_cost);
        }
        (RunDecision::Shop(shop), &RunAction::BuyPotion(i)) => {
            let entry = shop.potions.get(i)?;
            let slot = next.potions.iter_mut().find(|p| p.is_none())?;
            *slot = entry.id;
            next.gold -= shop::price(s, entry.base_cost);
        }
        _ => return None,
    }
    Some(next)
}

/// The run net's choice among `legal`, its top two re-ranked by the probe when close.
///
/// They are re-ranked when their logits are within [`ProbeUse::margin`]: each candidate's state (by [`after`], at full HP: the
/// deck is what is compared) against the act's boss, on the same worlds.
#[must_use]
pub fn choose(
    run_net: &RunNet,
    s: &RunState,
    decision: &RunDecision<'_>,
    legal: &[RunAction],
) -> Option<RunAction> {
    match PROBE.get() {
        Some((config, rollout)) => choose_with(*config, rollout, run_net, s, decision, legal),
        None => run_net.choose(s, decision, legal),
    }
}

/// [`choose`] with the probe `config` by the playout net `rollout`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    reason = "the run net's logits and a share of playouts; no game logic"
)]
pub fn choose_with(
    config: ProbeUse,
    rollout: &RolloutNet,
    run_net: &RunNet,
    s: &RunState,
    decision: &RunDecision<'_>,
    legal: &[RunAction],
) -> Option<RunAction> {
    let f = Features::of(s, decision, legal);
    let (logits, _) = run_net.evaluate(&f);
    let mut order: Vec<usize> = (0..logits.len()).collect();
    order.sort_by(|&a, &b| logits[b].total_cmp(&logits[a]));
    let pick = |i: usize| legal.get(f.candidates[i].position).cloned();
    let (&first, &second) = match order.as_slice() {
        [a, b, ..] if logits[*a] - logits[*b] < config.margin => (a, b),
        [a, ..] => return pick(*a),
        [] => return None,
    };
    let probed = |i: usize| {
        let action = pick(i)?;
        let mut state = after(s, decision, &action)?;
        state.hp = state.max_hp;
        let p = probe(&state, config.worlds, rollout);
        Some(logits[i] + config.weight * p.won as f32 / p.n.max(1) as f32)
    };
    let (Some(a), Some(b)) = (probed(first), probed(second)) else {
        return pick(first);
    };
    PROBED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if b > a {
        FLIPPED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return pick(second);
    }
    pick(first)
}
