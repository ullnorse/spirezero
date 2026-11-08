//! Powers: one file per power, each a unit struct implementing [`PowerModel`] (static data and
//! lifecycle) and [`Hooks`](crate::combat::hooks::Hooks) (listener hooks).
//!
//! To add a power: its file `<name>.rs` declaring `pub struct <Name>`; `build.rs` generates the
//! variant, `ALL`, `id` and `model` (see `docs/specs/content-api.md`, "Registration").

include!(concat!(env!("OUT_DIR"), "/content/powers.rs"));

mod display;
pub(crate) mod temporary;
pub(crate) mod temporary_dexterity;
pub(crate) mod temporary_focus;
pub(crate) mod temporary_strength;

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::state::Power;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::decimal::Decimal;

/// `PowerModel._internalData` (`InitInternalData`, `PowerModel.cs:520-534`).
///
/// Per-instance data hidden from the rest of the game and reset on clones. Read and written by
/// the power itself through [`CombatState::power`](crate::combat::CombatState) and `power_mut`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PowerData {
    /// `InitInternalData` returns `null`.
    #[default]
    None,
    /// `Illusion`'s fields ([`illusion::Data`]).
    Illusion(illusion::Data),
    /// `Vigor`'s fields ([`vigor::Data`]).
    Vigor(vigor::Data),
    /// `Oblivion`'s fields ([`oblivion::Data`]).
    Oblivion(oblivion::Data),
    /// `Slow`'s fields ([`slow::Data`]).
    Slow(slow::Data),
    /// `Calamity`'s fields ([`calamity::Data`]).
    Calamity(calamity::Data),
    /// `Automation`'s fields ([`automation::Data`]).
    Automation(automation::Data),
    /// `ToricToughness`'s fields ([`toric_toughness::Data`]).
    ToricToughness(toric_toughness::Data),
    /// `TheBomb`'s fields ([`the_bomb::Data`]).
    TheBomb(the_bomb::Data),
    /// `Panache`'s fields ([`panache::Data`]).
    Panache(panache::Data),
    /// `Ritual`'s fields ([`ritual::Data`]).
    Ritual(ritual::Data),
    /// `Gigantification`'s fields ([`gigantification::Data`]).
    Gigantification(gigantification::Data),
    /// `DarkEmbrace`'s fields ([`dark_embrace::Data`]).
    DarkEmbrace(dark_embrace::Data),
    /// `VoidForm`'s fields ([`void_form::Data`]).
    VoidForm(void_form::Data),
    /// `Rupture`'s fields ([`rupture::Data`]).
    Rupture(rupture::Data),
    /// `Inferno`'s fields ([`inferno::Data`]).
    Inferno(inferno::Data),
    /// `CrimsonMantle`'s fields ([`crimson_mantle::Data`]).
    CrimsonMantle(crimson_mantle::Data),
    /// `Juggling`'s fields ([`juggling::Data`]).
    Juggling(juggling::Data),
    /// `Feral`'s fields ([`feral::Data`]).
    Feral(feral::Data),
    /// `Afterimage`'s fields ([`afterimage::Data`]).
    Afterimage(afterimage::Data),
    /// `SerpentForm`'s fields ([`serpent_form::Data`]).
    SerpentForm(serpent_form::Data),
    /// `Nightmare`'s fields ([`nightmare::Data`]).
    Nightmare(nightmare::Data),
    /// `Strangle`'s fields ([`strangle::Data`]).
    Strangle(strangle::Data),
    /// `Storm`'s fields ([`storm::Data`]).
    Storm(storm::Data),
    /// `Orbit`'s fields ([`orbit::Data`]).
    Orbit(orbit::Data),
    /// `Monologue`'s fields ([`monologue::Data`]).
    Monologue(monologue::Data),
    /// `PaleBlueDot`'s fields ([`pale_blue_dot::Data`]).
    PaleBlueDot(pale_blue_dot::Data),
    /// `Subroutine`'s fields ([`subroutine::Data`]).
    Subroutine(subroutine::Data),
    /// `HardenedShell`'s fields ([`hardened_shell::Data`]).
    HardenedShell(hardened_shell::Data),
    /// `Hellraiser`'s fields ([`hellraiser::Data`]).
    Hellraiser(hellraiser::Data),
    /// `Skittish`'s fields ([`skittish::Data`]).
    Skittish(skittish::Data),
    /// `Thievery`'s fields ([`thievery::Data`]).
    Thievery(thievery::Data),
    /// `Heist`'s fields ([`heist::Data`]).
    Heist(heist::Data),
    /// `Tender`'s fields ([`tender::Data`]).
    Tender(tender::Data),
    /// `CurlUp`'s fields ([`curl_up::Data`]).
    CurlUp(curl_up::Data),
    /// `PossessStrength`'s fields ([`possess_strength::Data`]).
    PossessStrength(possess_strength::Data),
    /// `PossessSpeed`'s fields ([`possess_speed::Data`]).
    PossessSpeed(possess_speed::Data),
    /// `Swipe`'s fields ([`swipe::Data`]).
    Swipe(swipe::Data),
    /// `Reattach`'s fields ([`reattach::Data`]).
    Reattach(reattach::Data),
    /// `TestSwipe`'s fields ([`test_swipe::Data`]).
    #[cfg(test)]
    TestSwipe(test_swipe::Data),
    /// `Dampen`'s fields ([`dampen::Data`]).
    Dampen(dampen::Data),
    /// `Sloth`'s fields ([`sloth::Data`]).
    Sloth(sloth::Data),
    /// `Surrounded`'s fields ([`surrounded::Data`]).
    Surrounded(surrounded::Data),
    /// `Sandpit`'s fields ([`sandpit::Data`]).
    Sandpit(sandpit::Data),
    /// `ChainsOfBinding`'s fields ([`chains_of_binding::Data`]).
    ChainsOfBinding(chains_of_binding::Data),
    /// `Adaptable`'s fields ([`adaptable::Data`]).
    Adaptable(adaptable::Data),
    /// `Nemesis`'s fields ([`nemesis::Data`]).
    Nemesis(nemesis::Data),
    /// `WitheringPresence`'s fields ([`withering_presence::Data`]).
    WitheringPresence(withering_presence::Data),
}

/// `PowerCmd.TickDownDuration` (`PowerCmd.cs:194`): clears `SkipNextDurationTick` if it is set,
/// else decrements the power.
pub(crate) fn tick_down_duration(eng: &mut Engine<'_>, me: PowerRef) -> Flow {
    let power = eng.s.power_mut(me.owner, me.id);
    if power.skip_next_duration_tick {
        power.skip_next_duration_tick = false;
        Ok(())
    } else {
        eng.decrement_power(me)
    }
}

/// `PowerInstanceType` (`Entities.Powers/PowerInstanceType.cs`): how `PowerCmd.Apply` finds an
/// existing instance to stack onto (`FindExistingInstanceForStacking`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PowerInstanceType {
    /// One instance per creature; applying again stacks onto it.
    None,
    /// Every application adds a new instance (`TheBombPower`).
    Instanced,
    /// One instance per applier (`OblivionPower`).
    InstancedPerApplier,
}

/// `PowerType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PowerType {
    /// `PowerType.Buff`.
    Buff,
    /// `PowerType.Debuff`.
    Debuff,
}

/// A power's static data and lifecycle methods (`PowerModel`). Defaults match the C# base.
#[expect(
    unused_variables,
    reason = "default bodies ignore their arguments, like the C# virtual methods"
)]
pub(crate) trait PowerModel: Hooks<Me = PowerRef> {
    /// Whether this model can raise a player choice ([`crate::combat::DecisionRequest`]) from
    /// any of its entry points or hooks, directly or by creating, transforming into or playing a
    /// model that can. The combat snapshots a step only while such a model is in combat, so a
    /// wrong `false` panics at the choice (debug builds also check replay determinism).
    fn may_prompt(&self) -> bool {
        false
    }

    /// The English display name (`Title`).
    fn name(&self) -> &'static str;
    /// `Type`.
    fn power_type(&self) -> PowerType;

    /// `AllowNegative`: removed at exactly 0 instead of at `<= 0`.
    fn allows_negative(&self) -> bool {
        false
    }

    /// `InstanceType`.
    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::None
    }

    /// `ShouldPowerBeRemovedAfterOwnerDeath`.
    fn should_be_removed_after_owner_death(&self) -> bool {
        true
    }

    /// `ITemporaryPower.InternallyAppliedPower` for a `TemporaryStrengthPower`,
    /// `TemporaryDexterityPower` or `TemporaryFocusPower` subclass (`temporary.rs`); `None` for any other power.
    fn internally_applied_power(&self) -> Option<PowerKind> {
        None
    }

    /// `IsVisibleInternal`.
    fn is_visible(&self) -> bool {
        true
    }

    /// `ShouldOwnerDeathTriggerFatal` of `me`: whether killing its owner can trigger Fatal
    /// (`Feed`, `BookRepairKnife`); read through [`Engine::target_allows_fatal`].
    fn should_owner_death_trigger_fatal(&self, s: &CombatState, me: PowerRef) -> bool {
        true
    }

    /// `OwnerIsSecondaryEnemy`: the owner is a secondary enemy (`Creature.IsSecondaryEnemy`).
    fn owner_is_secondary_enemy(&self) -> bool {
        false
    }

    /// `InitInternalData`: the per-instance data of a new instance.
    fn init_internal_data(&self) -> PowerData {
        PowerData::None
    }

    /// `BeforeApplied(target, amount, applier)`, before a new instance is added.
    fn before_applied(
        &self,
        eng: &mut Engine<'_>,
        target: CreatureId,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        Ok(())
    }

    /// `AfterApplied(applier)`, after a new instance was added.
    fn after_applied(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        applier: Option<CreatureId>,
    ) -> Flow {
        Ok(())
    }

    /// `AfterRemoved(oldOwner)`. The instance is already off its owner (`RemoveInternal`), but
    /// as in the game it keeps its fields: `power` is the removed instance.
    fn after_removed(&self, eng: &mut Engine<'_>, power: &Power, old_owner: CreatureId) -> Flow {
        Ok(())
    }
}

impl PowerKind {
    /// [`PowerModel::before_applied`], statically dispatched (most powers keep the default).
    pub(crate) fn before_applied(
        self,
        eng: &mut Engine<'_>,
        target: CreatureId,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        with_model!(self, m => m.before_applied(eng, target, amount, applier))
    }

    /// [`PowerModel::after_applied`], statically dispatched (most powers keep the default).
    pub(crate) fn after_applied(
        self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        applier: Option<CreatureId>,
    ) -> Flow {
        with_model!(self, m => m.after_applied(eng, me, applier))
    }

    /// The English display name (`Title`).
    #[must_use]
    pub fn name(self) -> &'static str {
        with_model!(self, m => m.name())
    }

    /// Whether the model can raise a player choice (see `may_prompt` in the content API),
    /// statically dispatched: [`CombatState::may_prompt`](crate::combat::CombatState::may_prompt)
    /// asks every model on every step.
    #[must_use]
    pub fn may_prompt(self) -> bool {
        with_model!(self, m => m.may_prompt())
    }

    /// `Type`.
    #[must_use]
    pub fn power_type(self) -> PowerType {
        with_model!(self, m => m.power_type())
    }

    /// `AllowNegative`.
    #[must_use]
    pub fn allows_negative(self) -> bool {
        with_model!(self, m => m.allows_negative())
    }

    /// `InstanceType`.
    #[must_use]
    pub fn instance_type(self) -> PowerInstanceType {
        with_model!(self, m => m.instance_type())
    }

    /// `GetTypeForAmount(amount)` (`PowerModel.cs:468`); every power ported so far is a
    /// `Counter`.
    #[must_use]
    pub fn type_for_amount(self, amount: Decimal) -> PowerType {
        let power_type = self.power_type();
        if self.allows_negative() && amount < Decimal::ZERO {
            PowerType::Debuff
        } else if !self.allows_negative()
            && power_type == PowerType::Debuff
            && amount < Decimal::ZERO
        {
            PowerType::Buff
        } else {
            power_type
        }
    }

    /// The power implements `ITemporaryPower`.
    #[must_use]
    pub fn is_temporary(self) -> bool {
        with_model!(self, m => m.internally_applied_power()).is_some()
    }

    /// `ITemporaryPower.InternallyAppliedPower`.
    #[must_use]
    pub fn internally_applied_power(self) -> Option<Self> {
        with_model!(self, m => m.internally_applied_power())
    }

    /// `IsVisible` of a power on an enemy (`IsVisibleInternal`; a power on an ally that is not
    /// the local player is never visible).
    #[must_use]
    pub fn is_visible(self) -> bool {
        with_model!(self, m => m.is_visible())
    }

    /// `ShouldPowerBeRemovedAfterOwnerDeath`.
    #[must_use]
    pub fn should_be_removed_after_owner_death(self) -> bool {
        with_model!(self, m => m.should_be_removed_after_owner_death())
    }

    /// `ShouldOwnerDeathTriggerFatal` of the power `me` of this kind.
    #[must_use]
    pub fn should_owner_death_trigger_fatal(self, s: &CombatState, me: PowerRef) -> bool {
        with_model!(self, m => m.should_owner_death_trigger_fatal(s, me))
    }

    /// `OwnerIsSecondaryEnemy`.
    #[must_use]
    pub fn owner_is_secondary_enemy(self) -> bool {
        with_model!(self, m => m.owner_is_secondary_enemy())
    }

    /// `InitInternalData`.
    #[must_use]
    pub fn init_internal_data(self) -> PowerData {
        with_model!(self, m => m.init_internal_data())
    }
}
