//! The run observation's v2 section (RLN16, `RunOptions::obs_v2`).
//!
//! What a fight shows that the v1 bytes leave to the net to work out. Written at [`V2_AT`] (after the v1 sections, which do
//! not move) only with `obs_v2`, zero otherwise and outside a fight; [`V2_BYTES`] in all:
//!
//! - [`LINK_AT`]: per intent slot of the combat section ([`crate::obs::INTENTS`]), the creature
//!   slot of its enemy + 1 (`u8`, the creature tokens' `owner`), 0 for an empty slot.
//! - [`FORECAST_AT`]: per creature slot ([`CREATURES`]), its next [`STEPS`] moves after the one
//!   it shows ([`Hooked::forecast_into`], spire-mentor's `forecast_after_current`), each
//!   [`STEP_BYTES`]: the first intent's kind + 1 (`u8`, 0 for none), certainty (`u8`, the
//!   product of the random branches' weight shares × 255, rounded), the first intent's damage
//!   per hit and hits (`i16`, `u8`; 0 for a non-attack), and the move's whole attack (`i16`, the
//!   sum of damage × hits). The walk reads no random number: a random branch shows its
//!   heaviest allowed branch, with that branch's share as the certainty.
//! - [`PREVIEW_AT`]: per hand slot ([`HAND`]), [`PREVIEW_BYTES`]: the card's damage and block as
//!   its text shows them in the hand, not aimed (`i16` each; [`CombatState::card_preview`], the
//!   web's path: Osty deals Osty's attacks), its hits (`u8`, its `Repeat` var, else 1) and flags
//!   (`u8`: has a damage var, has a block var, from bit 0).
//! - [`AIMED_AT`]: per hand slot and creature slot, the card's damage per hit aimed at that
//!   creature (`i16`; 0 for allies and for cards without a damage var).
//! - [`SIDE_AT`] (RLN21): per creature slot, [`SIDE_BYTES`]: its facing (`u8`: 0 without
//!   `SurroundedPower`, 1 facing right, 2 facing left; the sprite shows it) and, for a crab arm
//!   (`BackAttackLeft`/`BackAttackRight`), whether it is behind the player (`u8`: 1 behind, its
//!   attacks on the player deal 1.5×; 2 in front; 0 otherwise).
//! - [`DISPLAY_AT`] (RLN21): per creature slot and power slot (the combat section's order),
//!   [`DISPLAY_BYTES`]: whether the power's icon shows a number other than its amount (`u8`, the
//!   C# `DisplayAmount` override) and that number (`i16`): Slow's damage bonus in %, Hardened
//!   Shell's HP loss left this turn, the counters of Orbit, Automation, Feral, Panache, Tender,
//!   Monologue, Sloth and Withering Presence.
//! - [`PLAN_AT`] (RLN22): per combat action index ([`crate::action::ACTIONS`]), [`PLAN_BYTES`]:
//!   the plan column's score of the action (`i16`, saturated; [`crate::plan_column`] at
//!   [`crate::plan_column::OBS_DEPTH`], on a blindfolded copy) and flags (`u8`: bit 0 scored,
//!   bit 1 the planner's choice, bit 2 a winning play). The run env writes it after the mask
//!   ([`crate::plan_column::write_plan`]), for the indices the mask holds; [`V2Observer`] leaves
//!   it zero.
//!
//! Everything here is a function of what the player sees (`tests/leak.rs` reseeds every stream).

use sts2_core::Combat;
use sts2_core::combat::state::Power;
use sts2_core::combat::{CombatState, CreatureId, Hooked, IntentType, PileType};
use sts2_core::content::cards::{PreviewMode, Var, VarClass};
use sts2_core::content::powers::{PowerData, PowerKind};

use crate::action::ACTIONS;
use crate::intents::{FORECAST_STEPS, IntentCache};
use crate::obs::{CREATURES, HAND, INTENTS, POWERS, Writer};

/// Forecast moves per creature.
pub const STEPS: usize = 3;
/// Bytes per forecast move.
pub const STEP_BYTES: usize = 7;
/// Bytes per hand card's preview.
pub const PREVIEW_BYTES: usize = 6;
/// Bytes per creature's facing.
pub const SIDE_BYTES: usize = 2;
/// Bytes per power's shown number.
pub const DISPLAY_BYTES: usize = 3;
/// Bytes per combat action's plan column entry.
pub const PLAN_BYTES: usize = 3;

/// Offsets inside the section.
pub const LINK_AT: usize = 0;
pub const FORECAST_AT: usize = LINK_AT + INTENTS;
pub const PREVIEW_AT: usize = FORECAST_AT + CREATURES * STEPS * STEP_BYTES;
pub const AIMED_AT: usize = PREVIEW_AT + HAND * PREVIEW_BYTES;
pub const SIDE_AT: usize = AIMED_AT + HAND * CREATURES * 2;
pub const DISPLAY_AT: usize = SIDE_AT + CREATURES * SIDE_BYTES;
pub const PLAN_AT: usize = DISPLAY_AT + CREATURES * POWERS * DISPLAY_BYTES;
/// Bytes of the section.
pub const V2_BYTES: usize = PLAN_AT + ACTIONS * PLAN_BYTES;

/// The section's offset in the run observation.
pub const V2_AT: usize = crate::run_obs::COMBAT_STATIC_AT + crate::obs::STATIC_BYTES;

const _: () = assert!(STEPS <= FORECAST_STEPS);

/// One env's writer of the v2 section, with its cache of the forecast moves' intents
/// ([`IntentCache::forecast_intents`]).
#[derive(Clone, Debug, Default)]
pub struct V2Observer {
    cache: IntentCache,
    /// With `obs_full`, what each enemy's moves showed ([`crate::obs_full`]).
    shown: crate::obs_full::Shown,
}

/// The creature slots of the observation: allies, then enemies, at most [`CREATURES`].
fn slots(s: &CombatState) -> impl Iterator<Item = (usize, CreatureId)> + '_ {
    s.allies
        .iter()
        .chain(&s.enemies)
        .copied()
        .take(CREATURES)
        .enumerate()
}

/// A certainty in `[0, 1]` as a byte.
#[allow(
    clippy::float_arithmetic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    reason = "the forecast's certainty is an f32 weight share; the byte is its rounding"
)]
fn certainty_byte(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

impl V2Observer {
    /// Starts a new episode or fight ([`IntentCache::reset`]).
    pub fn reset(&mut self) {
        self.cache.reset();
        self.shown.reset();
    }

    /// Writes the full-view section of `combat` into `out` ([`crate::obs_full`], RLN30), and
    /// records what each enemy's move shows now for when it is past. Call it after
    /// [`V2Observer::write`] of the same state, at every observation of a fight.
    ///
    /// # Panics
    /// If `out` is shorter than [`crate::obs_full::FULL_BYTES`].
    pub fn write_full(&mut self, combat: &Combat, out: &mut [u8]) {
        self.shown.write(&mut self.cache, combat, out);
    }

    /// Writes the section of `combat` into `out` ([`V2_BYTES`], zeroed first); `intent_owners`
    /// are the creatures of the combat section's intent slots, in order.
    ///
    /// # Panics
    /// If `out` is shorter than [`V2_BYTES`].
    pub fn write(&mut self, combat: &Combat, intent_owners: &[CreatureId], out: &mut [u8]) {
        let h = combat.hooked();
        let s = h.s;
        self.cache.begin();
        let mut w = Writer::new(&mut out[..V2_BYTES]);
        for &owner in intent_owners.iter().take(INTENTS) {
            w.u8(slots(s)
                .find(|&(_, id)| id == owner)
                .map_or(0, |(i, _)| i + 1));
        }
        for (slot, id) in slots(s) {
            w.seek(FORECAST_AT + slot * STEPS * STEP_BYTES);
            self.forecast(h, id, &mut w);
        }
        for (i, &card) in s.player.hand.iter().take(HAND).enumerate() {
            w.seek(PREVIEW_AT + i * PREVIEW_BYTES);
            let Some(damage) = preview(s, card, None, &mut w) else {
                continue;
            };
            if damage {
                for (slot, id) in slots(s) {
                    if s.enemies.contains(&id) && s.creature(id).is_alive() {
                        w.seek(AIMED_AT + (i * CREATURES + slot) * 2);
                        w.i16(aimed(s, card, id));
                    }
                }
            }
        }
        let facing = player_facing(s);
        for (slot, id) in slots(s) {
            let c = s.creature(id);
            w.seek(SIDE_AT + slot * SIDE_BYTES);
            w.u8(surrounded(s, &c.powers).map_or(0, |f| f as u8 + 1));
            let arm = if c.has_power(PowerKind::BackAttackLeft) {
                Some(Side::Left)
            } else if c.has_power(PowerKind::BackAttackRight) {
                Some(Side::Right)
            } else {
                None
            };
            w.u8(match (arm, facing) {
                (Some(arm), Some(facing)) if arm != facing => 1,
                (Some(_), Some(_)) => 2,
                _ => 0,
            });
            for (j, power) in c.powers.iter().take(POWERS).enumerate() {
                if let Some(shown) = display_amount(s, power) {
                    w.seek(DISPLAY_AT + (slot * POWERS + j) * DISPLAY_BYTES);
                    w.u8(1);
                    w.i16(shown);
                }
            }
        }
    }

    /// The forecast of creature `id` ([`STEPS`] × [`STEP_BYTES`]): its next moves
    /// ([`MoveMachine::forecast`], as [`Hooked::forecast_into`] walks them) with their intents
    /// from the cache.
    ///
    /// [`MoveMachine::forecast`]: sts2_core::content::monsters::machine::MoveMachine::forecast
    fn forecast(&mut self, h: Hooked<'_>, id: CreatureId, w: &mut Writer<'_>) {
        let c = h.s.creature(id);
        let Some(m) = c.monster.as_ref().filter(|_| c.is_alive()) else {
            return;
        };
        let moves = m.machine.forecast(m.kind.moves(), h.s, id, m.rolled, STEPS);
        for (step, &(state, certainty)) in moves.iter().enumerate() {
            let intents = self.cache.forecast_intents(h, id, step, state);
            let head = intents.first();
            let attack = head.and_then(|i| i.attack);
            w.u8(head.map_or(0, |i| i.kind as u8 + 1));
            w.u8(certainty_byte(certainty));
            w.i16(attack.map_or(0, |a| a.damage));
            w.u8(attack.map_or(0, |a| a.hits));
            let total = intents
                .iter()
                .filter(|i| matches!(i.kind, IntentType::Attack | IntentType::DeathBlow))
                .filter_map(|i| i.attack)
                .map(|a| i64::from(a.damage) * i64::from(a.hits))
                .sum::<i64>();
            w.i16(i32::try_from(total).unwrap_or(i32::MAX));
        }
    }
}

/// A side of the crab: the way a `SurroundedPower` owner faces, or the arm a `BackAttack` marks.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Right = 0,
    Left = 1,
}

/// The facing of the `SurroundedPower` among `powers`, if there is one. (`surrounded::Direction`
/// is not exported by `sts2-core`; its default is `Right`, the other variant `Left`.)
#[expect(
    clippy::default_trait_access,
    reason = "`surrounded::Direction` cannot be named outside sts2-core"
)]
fn surrounded(s: &CombatState, powers: &[Power]) -> Option<Side> {
    powers
        .iter()
        .find(|p| p.kind == PowerKind::Surrounded)
        .and_then(|p| match *s.power_data(p.id) {
            PowerData::Surrounded(d) if d.facing == Default::default() => Some(Side::Right),
            PowerData::Surrounded(_) => Some(Side::Left),
            _ => None,
        })
}

/// The player's facing: an arm on the other side is behind them.
fn player_facing(s: &CombatState) -> Option<Side> {
    surrounded(s, &s.creature(s.player.creature).powers)
}

/// The number `power`'s icon shows where it is not its amount (`DisplayAmount` overrides).
/// `PaleBlueDotPower` (attacks played this turn, from the history) is left out.
fn display_amount(s: &CombatState, power: &Power) -> Option<i32> {
    Some(match *s.power_data(power.id) {
        PowerData::Slow(d) => d.slow_amount * 10,
        PowerData::HardenedShell(d) => (power.amount - d.damage_received_this_turn).max(0),
        PowerData::Orbit(d) => 4 - d.energy_spent.rem_euclid(4),
        PowerData::Automation(d) => d.cards_left,
        PowerData::Feral(d) => (power.amount - d.zero_cost_attacks_played).max(0),
        PowerData::Panache(d) => d.cards_left,
        PowerData::Tender(d) => d.cards_played_this_turn,
        PowerData::Monologue(d) => d.strength_applied,
        PowerData::Sloth(d) => d.cards_played_this_turn,
        PowerData::WitheringPresence(d) => d.cards_left,
        _ => return None,
    })
}

/// Whether `class` is a damage var a play deals (not the extra step of a calculated one).
const fn is_damage(class: VarClass) -> bool {
    matches!(
        class,
        VarClass::Damage(_) | VarClass::OstyDamage(_) | VarClass::CalculatedDamage { .. }
    )
}

const fn is_block(class: VarClass) -> bool {
    matches!(class, VarClass::Block(_) | VarClass::CalculatedBlock(_))
}

/// Writes hand card `card`'s preview not aimed ([`PREVIEW_BYTES`]); whether it has a damage
/// var, or `None` for a card that is not in the hand.
fn preview(
    s: &CombatState,
    card: sts2_core::combat::CardId,
    target: Option<CreatureId>,
    w: &mut Writer<'_>,
) -> Option<bool> {
    if s.pile_of(card) != Some(PileType::Hand) {
        return None;
    }
    let vars = s.card(card).kind.vars();
    let values = s.card_preview(card, target, PreviewMode::Normal);
    let pick = |want: fn(VarClass) -> bool| {
        vars.iter()
            .zip(&values)
            .find(|(v, _)| want(v.class))
            .map(|(_, &(_, value))| value.to_int_truncating())
    };
    let damage = pick(is_damage);
    let block = pick(is_block);
    let hits = values
        .iter()
        .find(|&&(v, _)| v == Var::Repeat)
        .map_or(1, |&(_, value)| value.to_int_truncating());
    w.i16(damage.unwrap_or(0));
    w.i16(block.unwrap_or(0));
    w.u8(hits.max(0));
    w.u8(u8::from(damage.is_some()) | u8::from(block.is_some()) << 1);
    Some(damage.is_some())
}

/// Hand card `card`'s damage per hit aimed at `target`.
fn aimed(s: &CombatState, card: sts2_core::combat::CardId, target: CreatureId) -> i32 {
    let vars = s.card(card).kind.vars();
    let values = s.card_preview(card, Some(target), PreviewMode::Normal);
    vars.iter()
        .zip(&values)
        .find(|(v, _)| is_damage(v.class))
        .map_or(0, |(_, &(_, value))| value.to_int_truncating())
}
