//! The CB3 combat net playing the player's fights (WEB82).
//!
//! [`CombatNet`] answers the page's "the net's move here" with the move as the advisor shows one
//! (`{taps, label, aims, potion, index}`); the page lights it, then plays its `index`.
//!
//! The observation is the training env's ([`RunSight`]): the run is stepped one action at a time
//! from its start, and every position the env would show the agent is observed, so the full
//! view's past intents are the ones the net learned from. Only fights are answered.

use serde_json::{Value, json};
use sts2_core::combat::Action;
use sts2_core::run::{Room, RoomType, RunAction, RunDecision};
use sts2_rl::sight::RunSight;
use sts2_web::{Game, parse_modifiers};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::net::encode::encode;
use crate::net::{BadWeights, Net};
use crate::{parse_actions, shown};

/// The net and the player's run as its env sees it.
#[wasm_bindgen]
#[derive(Debug)]
pub struct CombatNet {
    net: Net,
    game: Option<Game>,
    sight: RunSight,
    /// The action count of the position last stepped ([`RunSight::step`]).
    stepped: Option<usize>,
}

/// The net's pick at a position.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    pub action: RunAction,
    /// The run-space index.
    pub index: usize,
    /// Its probability under the policy (1 for a forced move).
    pub prob: f32,
    /// Legal classes at the position.
    pub classes: u32,
}

impl CombatNet {
    /// The net from its weights file (`python/web82_export.py`).
    ///
    /// # Errors
    /// If the weights do not load.
    pub fn from_weights(weights: &[u8]) -> Result<Self, BadWeights> {
        Ok(Self {
            net: Net::from_bytes(weights)?,
            game: None,
            sight: RunSight::new(RunSight::cb3()),
            stepped: None,
        })
    }

    /// The run as the net's env sees it (the last position stepped), for the parity tests.
    #[must_use]
    pub const fn sight(&self) -> &RunSight {
        &self.sight
    }

    /// The net.
    #[must_use]
    pub const fn net(&self) -> &Net {
        &self.net
    }

    /// Steps the sight at the game's position, once per position.
    fn step_here(sight: &mut RunSight, stepped: &mut Option<usize>, game: &Game) {
        let at = game.actions().len();
        if *stepped != Some(at) {
            sight.step(game.run());
            *stepped = Some(at);
        }
    }

    /// The kept run moved to `actions`, stepping the sight at every position on the way.
    fn sync(
        &mut self,
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &[sts2_core::run::Modifier],
        actions: &[u32],
    ) -> Result<&Game, String> {
        let keep = self.game.as_ref().is_some_and(|g| {
            g.start() == (character, seed, ascension)
                && g.modifiers() == modifiers
                && actions.starts_with(g.actions())
        });
        if !keep {
            self.game = Some(Game::new(character, seed, ascension, modifiers)?);
            self.sight.reset();
            self.stepped = None;
        }
        let game = self.game.as_mut().ok_or("no run")?;
        for &a in &actions[game.actions().len()..] {
            Self::step_here(&mut self.sight, &mut self.stepped, game);
            game.advance(&[a])?;
        }
        Ok(game)
    }

    /// Whether the room is a hallway fight (a normal monster room), the one where potions are
    /// kept back under `save_potions`: read from the game state the player sees.
    const fn hallway(game: &Game) -> bool {
        matches!(
            game.run().state().room,
            Room::Combat {
                room_type: RoomType::Monster,
                ..
            }
        )
    }

    /// The net's pick at `game`'s position (which [`CombatNet::sync`] reached): `None` outside a
    /// fight or with nothing legal. With `save_potions`, a hallway fight's potion uses are not
    /// candidates (unless nothing else is legal).
    pub fn pick_at(&mut self, save_potions: bool) -> Option<Pick> {
        let game = self.game.as_ref()?;
        if game.is_over() || !matches!(game.run().pending_decision(), RunDecision::Combat(_)) {
            return None;
        }
        Self::step_here(&mut self.sight, &mut self.stepped, game);
        let classes = self.sight.mask().count();
        let (index, prob) = match classes {
            0 => return None,
            // The env plays a single class itself: the net is not asked.
            1 => (self.sight.mask().nth(0)?, 1.0),
            _ => {
                let tok = encode(self.sight.obs(), self.sight.mask().bits()).ok()?;
                let mut logits = self.net.logits(&tok);
                if save_potions && Self::hallway(game) {
                    let mut left = logits.clone();
                    for (i, l) in left.iter_mut().enumerate() {
                        let potion = matches!(
                            self.sight.action(i),
                            Some(RunAction::Combat(Action::UsePotion { .. }))
                        );
                        if potion {
                            *l = f32::NEG_INFINITY;
                        }
                    }
                    // Nothing but potions legal: the normal choice.
                    if Net::greedy(&left).is_some() {
                        logits = left;
                    }
                }
                let index = Net::greedy(&logits)?;
                (index, softmax_at(&logits, index))
            }
        };
        Some(Pick {
            action: self.sight.action(index)?.clone(),
            index,
            prob,
            classes,
        })
    }

    /// Plays the run `actions` (as [`Game::actions`] lists them) on to its next decision and
    /// the net's pick there, for the native tests and tools.
    ///
    /// # Errors
    /// If the run cannot be replayed.
    pub fn pick(
        &mut self,
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &[sts2_core::run::Modifier],
        actions: &[u32],
    ) -> Result<Option<Pick>, String> {
        self.pick_with(character, seed, ascension, modifiers, actions, false)
    }

    /// [`CombatNet::pick`] with `save_potions`: no potion use in a hallway fight.
    ///
    /// # Errors
    /// If the run cannot be replayed.
    pub fn pick_with(
        &mut self,
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &[sts2_core::run::Modifier],
        actions: &[u32],
        save_potions: bool,
    ) -> Result<Option<Pick>, String> {
        self.sync(character, seed, ascension, modifiers, actions)?;
        Ok(self.pick_at(save_potions))
    }
}

#[allow(
    clippy::float_arithmetic,
    reason = "the policy's probability of its pick, for the page's line"
)]
fn softmax_at(logits: &[f32], index: usize) -> f32 {
    let max = logits[index];
    let sum: f32 = logits
        .iter()
        .filter(|l| l.is_finite())
        .map(|l| (l - max).exp())
        .sum();
    1.0 / sum
}

#[wasm_bindgen]
impl CombatNet {
    /// The net from its weights file's bytes.
    ///
    /// # Errors
    /// If they are not a CB3 weights file.
    #[wasm_bindgen(constructor)]
    pub fn new(weights: &[u8]) -> Result<Self, String> {
        Self::from_weights(weights).map_err(|e| e.0)
    }

    /// The policy's logits (361, `-inf` where illegal) for one row: its observation bytes and
    /// its action mask's six words, as `python/web82_export.py dump` writes them (the wasm side
    /// of the parity check, `crates/sts2-web/tools/net_parity.mjs`).
    ///
    /// # Errors
    /// If the row is not a fight's.
    pub fn logits(&self, obs: &[u8], mask: &[u64]) -> Result<Vec<f32>, String> {
        let tok = encode(obs, mask).map_err(|e| format!("{e:?}"))?;
        Ok(self.net.logits(&tok))
    }

    /// The net's move in the run `character`/`seed`/`ascension`/`modifiers` after `actions`
    /// (the save's JSON), as JSON: the advisor's `{taps, label, aims, potion, index, fight}` plus
    /// `prob` (the policy's probability of it); `null` outside a fight. `save_potions` keeps
    /// the net from using potions in hallway fights (elites and bosses are unchanged).
    ///
    /// # Errors
    /// If the run cannot be replayed.
    pub fn advise(
        &mut self,
        character: usize,
        seed: &str,
        ascension: u8,
        modifiers: &str,
        actions: &str,
        save_potions: bool,
    ) -> Result<String, String> {
        let actions = parse_actions(actions)?;
        let modifiers = parse_modifiers(modifiers)?;
        let Some(pick) = self.pick_with(
            character,
            seed,
            ascension,
            &modifiers,
            &actions,
            save_potions,
        )?
        else {
            return Ok(Value::Null.to_string());
        };
        let game = self.game.as_ref().ok_or("no run")?;
        let mut v = shown(game, &pick.action, None);
        v["prob"] = json!(pick.prob);
        v["classes"] = json!(pick.classes);
        Ok(v.to_string())
    }
}
