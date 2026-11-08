//! The shared body of `TemporaryStrengthPower` and `TemporaryDexterityPower`
//! (`Models.Powers/TemporaryStrengthPower.cs`, `TemporaryDexterityPower.cs`; they differ only in
//! the stat, see `temporary_strength.rs` and `temporary_dexterity.rs`), the base of their subclasses (`FlexPotionPower`,
//! `EnfeeblingTouchPower`, ...), which never exist on their own. Each subclass keeps its
//! `Title`/`OriginModel` in its own file and forwards `BeforeApplied`, `AfterPowerAmountChanged`
//! and `AfterSideTurnEnd` here, with the stat it grants (`InternallyAppliedPower`) and `IsPositive`.
//!
//! The C# passes the applying card as `cardSource` to the inner stat's `PowerCmd.Apply`. Only
//! `UnsettlingLamp` reads it, and it has already doubled the outer temporary power for that card
//! (`HasDoubledTemporaryPowerSource` skips the inner one), so the inner apply goes without it.

use crate::combat::hooks::PowerRef;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::{PowerKind, PowerType};
use crate::decimal::Decimal;

/// A temporary stat power's parameters.
#[derive(Clone, Copy)]
pub(super) struct Temporary {
    /// `InternallyAppliedPower`: `Strength` or `Dexterity`.
    pub stat: PowerKind,
    /// `IsPositive`.
    pub positive: bool,
}

impl Temporary {
    pub const fn new(stat: PowerKind, positive: bool) -> Self {
        Self { stat, positive }
    }

    /// `Type`: a debuff unless positive.
    pub const fn power_type(self) -> PowerType {
        if self.positive {
            PowerType::Buff
        } else {
            PowerType::Debuff
        }
    }

    /// `Sign`.
    const fn sign(self) -> i32 {
        if self.positive { 1 } else { -1 }
    }

    /// `BeforeApplied`: applies `Sign * amount` of the stat to the target.
    pub fn before_applied(
        self,
        eng: &mut Engine<'_>,
        target: CreatureId,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        eng.apply_power(
            target,
            self.stat,
            Decimal::from(self.sign()) * amount,
            applier,
        )
    }

    /// `AfterPowerAmountChanged`: a stack of this power applies the change to the stat; the first
    /// application (the change is the whole amount) was done by `BeforeApplied`.
    pub fn after_power_amount_changed(
        self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        power: PowerRef,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        if amount == Decimal::from(eng.s.power(me.owner, me.id).amount) || power != me {
            return Ok(());
        }
        eng.apply_power(
            me.owner,
            self.stat,
            Decimal::from(self.sign()) * amount,
            applier,
        )
    }

    /// `AfterSideTurnEnd`: at the end of the owner's side's turn, remove the power and take the
    /// stat back.
    pub fn after_side_turn_end(
        self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            let amount = eng.s.power(me.owner, me.id).amount;
            eng.remove_power(me.owner, me.id)?;
            eng.apply_power(
                me.owner,
                self.stat,
                Decimal::from(-self.sign() * amount),
                Some(me.owner),
            )?;
        }
        Ok(())
    }
}
