//! `TemporaryFocusPower` (`Models.Powers/TemporaryFocusPower.cs`), the abstract base of
//! `FocusedStrikePower`, `HotfixPower`, `SynchronizePower` and `HyperbeamFocusDownPower`. It is
//! never instantiated: each subclass forwards its hooks to [`Temporary`] with its stat (Focus)
//! and `IsPositive`. The title is the `OriginModel`'s (the card that applies the power).

use super::temporary::Temporary;
use crate::content::powers::PowerKind;

/// A subclass that grants Focus (`IsPositive` true).
pub(super) const UP: Temporary = Temporary::new(PowerKind::Focus, true);
/// A subclass that takes Focus (`IsPositive` false).
pub(super) const DOWN: Temporary = Temporary::new(PowerKind::Focus, false);
