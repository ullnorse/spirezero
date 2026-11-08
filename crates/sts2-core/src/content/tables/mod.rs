//! Display tables generated from the decomp by `tools/gen_tables.py`.
//!
//! Each monster move's intents and the relic, potion, event and power `CanonicalVars`, for every model
//! of the game, ported or not.

mod event_vars;
mod intents;
mod potion_vars;
mod power_vars;
mod relic_vars;

use crate::decimal::Decimal;

/// One intent a `MoveState` is built with (`MonsterMoves.Intents/*.cs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Declared {
    SingleAttack,
    MultiAttack,
    DeathBlow,
    Buff,
    Debuff,
    /// `DebuffIntent(strong: true)`.
    DebuffStrong,
    Defend,
    Escape,
    Heal,
    Hidden,
    Summon,
    Sleep,
    Stun,
    /// `StatusIntent(count)`.
    StatusCard(CardCount),
    CardDebuff,
}

/// A `StatusIntent`'s `CardCount`, as the monster computes it when it builds its moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CardCount {
    Fixed(u32),
    /// `AscensionHelper.GetValueIfAscension(level, then, otherwise)`.
    Ascension {
        level: u8,
        then: u32,
        otherwise: u32,
    },
}

impl CardCount {
    /// The count at `ascension`.
    pub(crate) const fn at(self, ascension: u8) -> u32 {
        match self {
            Self::Fixed(n) => n,
            Self::Ascension {
                level,
                then,
                otherwise,
            } => {
                if ascension >= level {
                    then
                } else {
                    otherwise
                }
            }
        }
    }
}

/// One `CanonicalVars` entry of a relic or potion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModelVar {
    /// `DynamicVar.Name`, the placeholder in the model's text (`Heal`, `StrengthPower`).
    pub name: &'static str,
    /// The canonical `BaseValue`; `None` for a `StringVar` or a base the model sets itself.
    pub value: Option<Decimal>,
    /// A `StringVar`'s fixed text, if it has one.
    pub text: Option<VarText>,
}

/// The fixed text of a `StringVar`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VarText {
    /// `ModelDb.Card<T>().Title`, by `ModelId.Entry`.
    Card(&'static str),
    /// `ModelDb.Relic<T>().Title`.
    Relic(&'static str),
    /// `ModelDb.Potion<T>().Title`.
    Potion(&'static str),
    /// `ModelDb.Enchantment<T>().Title`.
    Enchantment(&'static str),
    /// `ModelDb.Affliction<T>().Title`.
    Affliction(&'static str),
    /// A string literal.
    Literal(&'static str),
    /// `EndlessConveyor`'s dish title (`ENDLESS_CONVEYOR.DISHES.<id>.title` in `events`).
    Dish(&'static str),
    /// The raw text of an `events` key (`L10NLookup(key).GetRawText()`: `Trial`'s story).
    Event(&'static str),
    /// A field of the player's character in `characters` (`{CHARACTER}.aromaPrinciple`).
    Character(&'static str),
    /// A relic's description (`ModelDb.Relic<T>().DynamicDescription`), by `ModelId.Entry`.
    RelicDescription(&'static str),
}

/// The intents the `MoveState` `move_id` of monster `monster` (`ModelId.Entry`) is built with.
pub(crate) fn declared_intents(monster: &str, move_id: &str) -> Option<&'static [Declared]> {
    intents::INTENTS
        .binary_search_by(|&(m, id, _)| (m, id).cmp(&(monster, move_id)))
        .ok()
        .map(|i| intents::INTENTS[i].2)
}

fn lookup(table: &'static [(&str, &[ModelVar])], id: &str) -> &'static [ModelVar] {
    table
        .binary_search_by(|&(model, _)| model.cmp(id))
        .map_or(&[], |i| table[i].1)
}

/// A relic's `CanonicalVars` by `ModelId.Entry` (empty for none).
pub(crate) fn relic_vars(id: &str) -> &'static [ModelVar] {
    lookup(relic_vars::RELIC_VARS, id)
}

/// An event's `CanonicalVars` by `ModelId.Entry` (empty for none).
pub(crate) fn event_vars(id: &str) -> &'static [ModelVar] {
    lookup(event_vars::EVENT_VARS, id)
}

/// A power's `CanonicalVars` by `ModelId.Entry` (`VULNERABLE_POWER`; empty for none).
pub(crate) fn power_vars(id: &str) -> &'static [ModelVar] {
    lookup(power_vars::POWER_VARS, id)
}

/// A potion's `CanonicalVars` by `ModelId.Entry` (empty for none).
pub(crate) fn potion_vars(id: &str) -> &'static [ModelVar] {
    lookup(potion_vars::POTION_VARS, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_sorted_for_binary_search() {
        assert!(
            intents::INTENTS
                .windows(2)
                .all(|w| (w[0].0, w[0].1) < (w[1].0, w[1].1))
        );
        assert!(relic_vars::RELIC_VARS.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(potion_vars::POTION_VARS.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(event_vars::EVENT_VARS.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(power_vars::POWER_VARS.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn lookups_find_declarations() {
        assert_eq!(
            declared_intents("GAS_BOMB", "EXPLODE_MOVE"),
            Some(&[Declared::DeathBlow][..])
        );
        assert_eq!(
            relic_vars("ANCHOR"),
            &[ModelVar {
                name: "Block",
                value: Some(Decimal::from(10)),
                text: None,
            }]
        );
        assert_eq!(relic_vars("NOT_A_RELIC"), &[]);
    }
}
