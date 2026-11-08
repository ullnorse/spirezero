//! What a power's text shows: its `DynamicVars` (`CanonicalVars` and the bases the power keeps
//! in its fields). Display only.

use super::{PowerData, PowerKind};
use crate::content::tables::{self, ModelVar};
use crate::decimal::Decimal;

impl PowerKind {
    /// `CanonicalVars` (with `None` for a `StringVar`).
    #[must_use]
    pub fn canonical_vars(self) -> &'static [ModelVar] {
        tables::power_vars(self.id())
    }

    /// `DynamicVars` of an instance with `data`, by name, as its text shows them: the canonical
    /// ones with the bases the power sets (`SlowPower`'s `SlowAmount`, `TheBombPower`'s
    /// `Damage`, ...). A `StringVar` has no number and is left out.
    #[must_use]
    pub fn vars(self, data: &PowerData) -> Vec<(&'static str, Decimal)> {
        self.canonical_vars()
            .iter()
            .filter_map(|v| {
                let set = match (data, v.name) {
                    (PowerData::Slow(d), "SlowAmount") => Some(Decimal::from(d.slow_amount)),
                    // `DisplayAmount = SlowAmount * 10` (`SlowPower.cs:35`).
                    (PowerData::Slow(d), "DisplayAmount") => {
                        Some(Decimal::from(d.slow_amount * 10))
                    }
                    (PowerData::TheBomb(d), "Damage") => Some(d.damage),
                    (PowerData::ToricToughness(d), "Block") => Some(d.block),
                    (PowerData::Panache(d), "CardsLeft") => Some(Decimal::from(d.cards_left)),
                    (PowerData::WitheringPresence(d), "CardsLeft") => {
                        Some(Decimal::from(d.cards_left))
                    }
                    (PowerData::Monologue(d), "StrengthPower") => Some(Decimal::from(d.strength)),
                    (PowerData::Monologue(d), "StrengthApplied") => {
                        Some(Decimal::from(d.strength_applied))
                    }
                    (PowerData::Thievery(d), "Gold") => Some(Decimal::from(d.gold)),
                    _ => None,
                };
                Some((v.name, set.or(v.value)?))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::powers::slow;

    #[test]
    fn vars_show_the_instance_bases() {
        assert_eq!(
            PowerKind::Vulnerable.vars(&PowerData::None),
            vec![("DamageIncrease", Decimal::new(15, 1))]
        );
        let slow = PowerData::Slow(slow::Data { slow_amount: 2 });
        assert_eq!(
            PowerKind::Slow.vars(&slow),
            vec![
                ("SlowAmount", Decimal::from(2)),
                ("DisplayAmount", Decimal::from(20))
            ]
        );
    }
}
