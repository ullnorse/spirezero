//! Fixtures: a seed, a referee actions file and the referee's trace for it, kept so the engine
//! can be replayed against the real game's answer without running the game.

use std::path::Path;

use serde_json::{Value, json};

use sts2_core::Character;
use sts2_core::combat::Encounter;

use crate::Result;
use crate::compare::{Divergence, first_divergence, first_run_divergence};
use crate::run_session;
use crate::session::{ASCENSION, character_or_default, encounter_or_default, replay};

/// A loaded fixture.
#[derive(Clone, Debug)]
pub struct Fixture {
    pub seed: String,
    /// The referee actions file (`character`, `encounter`, `actions`, ...).
    pub actions: Value,
    /// The referee's trace lines.
    pub referee_trace: Vec<Value>,
    /// Where the engine was known to diverge when the fixture was made, if it did.
    pub divergence: Option<Value>,
}

impl Fixture {
    /// Reads a fixture file.
    ///
    /// # Errors
    /// If the file is missing or malformed.
    pub fn load(path: &Path) -> Result<Self> {
        let value: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let referee_trace = value["referee_trace"]
            .as_array()
            .ok_or("fixture has no referee_trace")?
            .clone();
        Ok(Self {
            seed: value["seed"]
                .as_str()
                .ok_or("fixture has no seed")?
                .to_owned(),
            actions: value["actions"].clone(),
            referee_trace,
            divergence: value.get("divergence").filter(|d| !d.is_null()).cloned(),
        })
    }

    /// The first divergence of the engine from the referee trace, if any.
    ///
    /// # Errors
    /// If the engine cannot replay the actions.
    pub fn check(&self) -> Result<Option<Divergence>> {
        let actions = self.actions["actions"]
            .as_array()
            .ok_or("fixture actions has no actions list")?;
        let rust = replay(
            character_of(&self.actions)?,
            &self.seed,
            encounter_of(&self.actions)?,
            ascension_of(&self.actions)?,
            self.actions.get("loadout"),
            actions,
        )?;
        Ok(first_divergence(&rust, &self.referee_trace))
    }
}

/// Builds the fixture for `seed`: the shortest action prefix that shows the first divergence
/// (all of it if there is none), with the referee trace cut to match.
///
/// # Errors
/// If the engine cannot replay the actions.
pub fn build(seed: &str, actions_file: &Value, referee_trace: &[Value]) -> Result<Value> {
    let actions = actions_file["actions"]
        .as_array()
        .ok_or("actions file has no actions list")?;
    let rust = replay(
        character_of(actions_file)?,
        seed,
        encounter_of(actions_file)?,
        ascension_of(actions_file)?,
        actions_file.get("loadout"),
        actions,
    )?;
    let divergence = first_divergence(&rust, referee_trace);
    let steps = divergence.as_ref().map_or(rust.len(), |d| d.step + 1);
    let mut file = actions_file.clone();
    file["actions"] = Value::Array(actions.iter().take(steps - 1).cloned().collect());
    Ok(json!({
        "seed": seed,
        "divergence": divergence.map(|d| json!({
            "step": d.step,
            "path": d.path,
            "rust": d.rust,
            "referee": d.referee,
        })),
        "actions": file,
        "referee_trace": referee_trace.iter().take(steps).collect::<Vec<_>>(),
    }))
}

/// The character an actions file plays (the default when it names none).
///
/// # Errors
/// If it names no character.
pub fn character_of(actions_file: &Value) -> Result<Character> {
    character_or_default(actions_file["character"].as_str())
}

/// The encounter an actions file forces (the default when it names none).
///
/// # Errors
/// If it names an encounter the engine does not have.
pub fn encounter_of(actions_file: &Value) -> Result<Encounter> {
    encounter_or_default(actions_file["encounter"].as_str())
}

/// The ascension an actions file plays at (0 when it names none).
///
/// # Errors
/// If it is not a small number.
pub fn ascension_of(actions_file: &Value) -> Result<u8> {
    actions_file.get("ascension").map_or(Ok(ASCENSION), |a| {
        a.as_u64()
            .and_then(|a| u8::try_from(a).ok())
            .ok_or_else(|| format!("bad ascension {a}").into())
    })
}

/// The directory fixtures live in.
#[must_use]
pub fn dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// A run-mode fixture: a seed, a run-mode referee actions file (`"mode": "run"`) and the
/// referee's trace for it, replayed with [`run_session::replay`].
#[derive(Clone, Debug)]
pub struct RunFixture {
    pub seed: String,
    /// The referee actions file.
    pub actions: Value,
    /// The referee's trace lines.
    pub referee_trace: Vec<Value>,
}

impl RunFixture {
    /// Reads a run fixture file.
    ///
    /// # Errors
    /// If the file is missing or malformed.
    pub fn load(path: &Path) -> Result<Self> {
        let value: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        Ok(Self {
            seed: value["seed"]
                .as_str()
                .ok_or("run fixture has no seed")?
                .to_owned(),
            actions: value["actions"].clone(),
            referee_trace: value["referee_trace"]
                .as_array()
                .ok_or("run fixture has no referee_trace")?
                .clone(),
        })
    }

    /// The first divergence of the engine from the referee trace, if any.
    ///
    /// # Errors
    /// If the engine cannot replay the actions.
    pub fn check(&self) -> Result<Option<Divergence>> {
        let actions = self.actions["actions"]
            .as_array()
            .ok_or("run fixture actions has no actions list")?;
        let rust = run_session::replay(
            character_of(&self.actions)?,
            &self.seed,
            ascension_of(&self.actions)?,
            (
                self.actions["real_shop_prices"].as_bool() == Some(true),
                run_session::act1_of(&self.actions)?,
            ),
            actions,
        )?;
        Ok(first_run_divergence(&rust, &self.referee_trace))
    }
}

/// Builds a run fixture for `seed` from the first `steps` actions (all of them for `None`), with
/// the referee trace cut to match.
///
/// # Errors
/// If the actions file has no actions list.
pub fn build_run(
    seed: &str,
    actions_file: &Value,
    referee_trace: &[Value],
    steps: Option<usize>,
) -> Result<Value> {
    let actions = actions_file["actions"]
        .as_array()
        .ok_or("actions file has no actions list")?;
    let steps = steps.unwrap_or(actions.len()).min(actions.len());
    let mut file = actions_file.clone();
    file["actions"] = Value::Array(actions.iter().take(steps).cloned().collect());
    Ok(json!({
        "seed": seed,
        "actions": file,
        "referee_trace": referee_trace.iter().take(steps + 1).collect::<Vec<_>>(),
    }))
}

/// The directory run fixtures live in.
#[must_use]
pub fn run_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("run_fixtures")
}
