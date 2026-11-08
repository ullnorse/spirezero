//! `TemporaryDexterityPower` (`Models.Powers/TemporaryDexterityPower.cs`), the abstract base of
//! `SpeedPotionPower` (and the Dexterity cards). It is never instantiated: each subclass forwards
//! its hooks to [`Temporary`] with its stat (Dexterity) and `IsPositive`.

use super::temporary::Temporary;
use crate::content::powers::PowerKind;

/// A subclass that grants Dexterity (`IsPositive` true).
pub(super) const UP: Temporary = Temporary::new(PowerKind::Dexterity, true);
