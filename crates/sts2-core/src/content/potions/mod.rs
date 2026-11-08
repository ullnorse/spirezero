//! Potions: one file per potion, each a unit struct implementing [`PotionModel`] (static data
//! and `OnUse`) and [`Hooks`](crate::combat::hooks::Hooks).
//!
//! To add a potion: its file `<name>.rs` declaring `pub struct <Name>`; `build.rs` generates the
//! variant, `ALL`, `id` and `model` (see `docs/specs/content-api.md`, "Registration").

include!(concat!(env!("OUT_DIR"), "/content/potions.rs"));

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::run::{RunError, RunState};

/// `PotionUsage`: when the player may use the potion manually.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PotionUsage {
    /// Only during the player's play phase in combat.
    CombatOnly,
    /// In and out of combat.
    AnyTime,
    /// Never manually; the potion triggers itself.
    Automatic,
}

/// A potion's static data and `OnUse` (`PotionModel`).
pub(crate) trait PotionModel: Hooks<Me = PotionId> {
    /// Whether this model can raise a player choice ([`crate::combat::DecisionRequest`]) from
    /// any of its entry points or hooks, directly or by creating, transforming into or playing a
    /// model that can. The combat snapshots a step only while such a model is in combat, so a
    /// wrong `false` panics at the choice (debug builds also check replay determinism).
    fn may_prompt(&self) -> bool {
        false
    }

    /// The English `Title`.
    fn name(&self) -> &'static str;
    /// `Usage`.
    fn usage(&self) -> PotionUsage;
    /// `TargetType`.
    fn target(&self) -> TargetType;
    /// `OnUse(choiceContext, target)`; `target` was checked by `IsValidTarget`.
    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow;

    /// `OnUse` outside combat, on the player (an `AnyTime` potion, or `FairyInABottle` preventing
    /// a death; its target is the player).
    ///
    /// # Errors
    /// If the effect meets content that is not ported.
    ///
    /// # Panics
    /// For any other potion (the run never uses one outside combat).
    fn on_use_out_of_combat(&self, _s: &mut RunState) -> Result<(), RunError> {
        unreachable!("{} is not used outside combat", self.name())
    }

    /// `PassesCustomUsabilityCheck` outside combat (an `AnyTime` potion).
    fn usable_out_of_combat(&self, _s: &RunState) -> bool {
        true
    }
}

impl PotionKind {
    /// `DynamicVars` by name, as the potion's text shows them (`CanonicalVars`; no potion
    /// changes its vars). A `StringVar` has no number and is left out.
    #[must_use]
    pub fn vars(self) -> Vec<(&'static str, crate::decimal::Decimal)> {
        crate::content::tables::potion_vars(self.id())
            .iter()
            .filter_map(|v| Some((v.name, v.value?)))
            .collect()
    }

    /// The English display name (`Title`).
    #[must_use]
    pub fn name(self) -> &'static str {
        with_model!(self, m => m.name())
    }

    /// Whether the model can raise a player choice (see `may_prompt` in the content API).
    #[must_use]
    pub fn may_prompt(self) -> bool {
        with_model!(self, m => m.may_prompt())
    }

    /// `Usage`.
    #[must_use]
    pub fn usage(self) -> PotionUsage {
        with_model!(self, m => m.usage())
    }

    /// `TargetType`.
    #[must_use]
    pub fn target(self) -> TargetType {
        with_model!(self, m => m.target())
    }

    /// `PassesCustomUsabilityCheck` outside combat.
    #[must_use]
    pub fn usable_out_of_combat(self, s: &RunState) -> bool {
        with_model!(self, m => m.usable_out_of_combat(s))
    }

    /// `OnUseWrapper` outside combat: the potion's `OnUse` on the player.
    ///
    /// # Errors
    /// If the effect meets content that is not ported.
    pub fn use_out_of_combat(self, s: &mut RunState) -> Result<(), RunError> {
        with_model!(self, m => m.on_use_out_of_combat(s))
    }
}

/// `PotionModel.OnUse`.
pub(crate) fn on_use(eng: &mut Engine<'_>, kind: PotionKind, target: Option<CreatureId>) -> Flow {
    kind.model().on_use(eng, target)
}
