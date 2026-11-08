//! `TemporaryStrengthPower` (`Models.Powers/TemporaryStrengthPower.cs`), the abstract base of
//! `EnfeeblingTouchPower`, `FlexPotionPower`, `ReptileTrinketPower`, ... It is never instantiated:
//! each subclass forwards its hooks to [`Temporary`] with its stat (Strength) and `IsPositive`.

use super::temporary::Temporary;
use crate::content::powers::PowerKind;

/// A subclass that grants Strength (`IsPositive` true).
pub(super) const UP: Temporary = Temporary::new(PowerKind::Strength, true);
/// A subclass that takes Strength (`IsPositive` false).
pub(super) const DOWN: Temporary = Temporary::new(PowerKind::Strength, false);
