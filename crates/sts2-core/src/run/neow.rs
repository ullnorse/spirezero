//! Neow's heal (`Models/AncientEventModel.cs`), which the combat and the TUI also show before a
//! run starts. Neow's options are in `content/ancients/neow.rs`.

use super::ascension::AscensionLevel;
use crate::decimal::Decimal;

/// The player's HP after Neow's `BeforeEventStarted`.
///
/// `Models/AncientEventModel.cs:170-191`: HP is set to 0, then healed by the missing HP, ×0.8
/// from `WearyTraveler` on; `Heal` truncates (66 max HP gives 52 at ascension 2).
#[must_use]
pub fn hp_after_neow(max_hp: i32, ascension: u8) -> i32 {
    let mut amount = Decimal::from(max_hp);
    if AscensionLevel::WearyTraveler.is_active(ascension) {
        amount *= Decimal::new(8, 1);
    }
    amount.min(Decimal::from(max_hp)).to_int_truncating()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weary_traveler_heals_80_percent_at_neow() {
        assert_eq!(hp_after_neow(66, 0), 66);
        assert_eq!(hp_after_neow(66, 1), 66);
        assert_eq!(hp_after_neow(66, 2), 52);
    }
}
