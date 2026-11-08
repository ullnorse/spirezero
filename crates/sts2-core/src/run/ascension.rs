//! `AscensionLevel` (`Entities.Ascension/AscensionLevel.cs`) and
//! `AscensionManager.HasLevel`: a level is active from its number on.

/// The ascension levels a run can start at (`AscensionManager.maxAscensionAllowed`).
pub const MAX_ASCENSION: u8 = 10;

/// `AscensionLevel`, numbered as in the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AscensionLevel {
    /// `AscensionLevel.SwarmingElites`.
    SwarmingElites = 1,
    /// `AscensionLevel.WearyTraveler`.
    WearyTraveler = 2,
    /// `AscensionLevel.Poverty`.
    Poverty = 3,
    /// `AscensionLevel.TightBelt`.
    TightBelt = 4,
    /// `AscensionLevel.AscendersBane`.
    AscendersBane = 5,
    /// `AscensionLevel.Inflation`.
    Inflation = 6,
    /// `AscensionLevel.Scarcity`.
    Scarcity = 7,
    /// `AscensionLevel.ToughEnemies`.
    ToughEnemies = 8,
    /// `AscensionLevel.DeadlyEnemies`.
    DeadlyEnemies = 9,
    /// `AscensionLevel.DoubleBoss`.
    DoubleBoss = 10,
}

impl_index!(AscensionLevel);

impl AscensionLevel {
    /// `AscensionManager.HasLevel`: `_level >= (int)level`.
    #[must_use]
    #[expect(
        clippy::as_conversions,
        reason = "const fn: the level's discriminant is its number"
    )]
    pub const fn is_active(self, ascension: u8) -> bool {
        ascension >= self as u8
    }
}
