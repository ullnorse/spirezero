//! `Flight` (`Models.Modifiers/Flight.cs`): `ShouldAllowFreeTravel` is always true, so the map
//! offers every point of the next row ([`crate::run::hooks::should_allow_free_travel`]).
//! Nothing else: the `WingedBoots` charge count is the relic's own.

/// `Flight.ShouldAllowFreeTravel`.
pub(super) const fn allows_free_travel() -> bool {
    true
}
