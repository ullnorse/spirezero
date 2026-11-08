//! Card `DynamicVars` (`Localization.DynamicVars/DynamicVarSet.cs`): the named numbers a card's
//! text shows and its `OnPlay` reads.
//!
//! A card declares its vars once in [`CardModel::vars`](super::CardModel::vars), in
//! `CanonicalVars` order, each with its canonical `BaseValue` and its `OnUpgrade`
//! `UpgradeValueBy` step. An instance's value is the canonical one at its upgrade level, adjusted
//! by the model's [`instance_var`](super::CardModel::instance_var) for vars the card class mutates
//! (`Rampage`'s growth, `SovereignBlade`'s forged damage). A calculated var
//! (`CalculatedVar.WithMultiplier`) is `CalculationBase + extra * multiplier`
//! ([`CombatState::calculate`]).

use std::fmt;

use crate::combat::hooks::{BlockQuery, DamageQuery};
use crate::combat::state::Card;
use crate::combat::{CardId, CombatState, CreatureId, Creatures, PileType, ValueProps};
use crate::content::cards::TargetType;
use crate::content::cards::{CardKind, CardModel};
use crate::content::enchantments::{Enchantment, EnchantmentKind, tezcataras_ember};
use crate::content::powers::PowerKind;
use crate::decimal::{Decimal, PackedDecimal};

/// A `DynamicVar`'s key in its card's `DynamicVarSet`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Var {
    /// `DamageVar`.
    Damage,
    /// `BlockVar`.
    Block,
    /// `CardsVar`.
    Cards,
    /// `EnergyVar`.
    Energy,
    /// `RepeatVar`.
    Repeat,
    /// `StarsVar`.
    Stars,
    /// `SummonVar`.
    Summon,
    /// `OstyDamageVar`.
    OstyDamage,
    /// `HpLossVar`.
    HpLoss,
    /// `ForgeVar`.
    Forge,
    /// `GoldVar`.
    Gold,
    /// `MaxHpVar`.
    MaxHp,
    /// `HealVar`.
    Heal,
    /// `CalculationBaseVar`: the base of the card's calculated var.
    CalculationBase,
    /// `CalculationExtraVar`: the per-multiplier step of a calculated var other than
    /// `CalculatedDamage`.
    CalculationExtra,
    /// `ExtraDamageVar`: the per-multiplier step of `CalculatedDamage`.
    ExtraDamage,
    /// `CalculatedDamageVar`.
    CalculatedDamage,
    /// `CalculatedBlockVar`.
    CalculatedBlock,
    /// `PowerVar<T>`, keyed by the power's class name (`VulnerablePower`).
    Power(PowerKind),
    /// A var with an explicit name (`new DynamicVar("Increase", 5m)`,
    /// `new CalculatedVar("CalculatedHits")`).
    Named(&'static str),
}

impl Var {
    /// The extra var of a calculated var (`CalculatedVar.GetExtraVar`): `ExtraDamage` for
    /// `CalculatedDamageVar`, `CalculationExtra` for the others.
    const fn extra(self) -> Self {
        match self {
            Self::CalculatedDamage => Self::ExtraDamage,
            _ => Self::CalculationExtra,
        }
    }
}

/// The key as the game names it (`DynamicVar.Name`, the placeholder in the card text).
impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Power(kind) => write!(f, "{kind:?}Power"),
            Self::Named(name) => f.write_str(name),
            other => write!(f, "{other:?}"),
        }
    }
}

/// `CalculatedVar`'s multiplier (`WithMultiplier`): the card, and the target (`None` for no
/// single target).
pub type Multiplier = fn(&CombatState, CardId, Option<CreatureId>) -> Decimal;

/// The `DynamicVar` subclass of a declared var, which decides its `UpdateCardPreview`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarClass {
    /// `DynamicVar` and the subclasses that preview their `BaseValue` (`SummonVar`'s
    /// `Hook.ModifySummonAmount` has no listener in the game).
    Plain,
    /// `DamageVar` with its `Props`.
    Damage(ValueProps),
    /// `BlockVar` with its `Props`.
    Block(ValueProps),
    /// `OstyDamageVar` with its `Props`.
    OstyDamage(ValueProps),
    /// `ExtraDamageVar`.
    ExtraDamage,
    /// `CalculatedDamageVar` with its `Props` and `IsFromOsty`.
    CalculatedDamage {
        /// `Props`.
        props: ValueProps,
        /// `IsFromOsty`.
        osty: bool,
    },
    /// `CalculatedBlockVar` with its `Props`.
    CalculatedBlock(ValueProps),
    /// Any other `CalculatedVar`.
    Calculated,
}

/// One declared var: a `CanonicalVars` entry plus its `OnUpgrade` step.
#[derive(Clone, Copy, Debug)]
pub struct CardVar {
    /// The var's name.
    pub name: Var,
    /// The canonical `BaseValue` (a calculated var's is its `CalculationBase`'s).
    pub base: Decimal,
    /// `UpgradeValueBy` in `OnUpgrade` (every card has at most one upgrade level).
    pub upgrade: Decimal,
    /// `WithMultiplier`, for a calculated var.
    pub multiplier: Option<Multiplier>,
    /// The var's class: the class the game's default name stands for (`DamageVar` for
    /// `Damage`, with `ValueProp.Move`), unless the card declares another.
    pub class: VarClass,
}

impl CardVar {
    /// A var with an integral base value.
    #[must_use]
    pub const fn new(name: Var, base: i32) -> Self {
        Self::decimal(name, Decimal::from_i32(base))
    }

    /// A var with any base value (`new DynamicVar("DamageIncrease", 1.5m)`).
    #[must_use]
    pub const fn decimal(name: Var, base: Decimal) -> Self {
        let class = match name {
            Var::Damage => VarClass::Damage(ValueProps::MOVE),
            Var::Block => VarClass::Block(ValueProps::MOVE),
            Var::OstyDamage => VarClass::OstyDamage(ValueProps::MOVE),
            Var::ExtraDamage => VarClass::ExtraDamage,
            _ => VarClass::Plain,
        };
        Self {
            name,
            base,
            upgrade: Decimal::ZERO,
            multiplier: None,
            class,
        }
    }

    /// A calculated var (`new CalculatedDamageVar(..).WithMultiplier(multiplier)`); declare its
    /// `CalculationBase` and extra var too.
    #[must_use]
    pub const fn calculated(name: Var, multiplier: Multiplier) -> Self {
        let class = match name {
            Var::CalculatedDamage => VarClass::CalculatedDamage {
                props: ValueProps::MOVE,
                osty: false,
            },
            Var::CalculatedBlock => VarClass::CalculatedBlock(ValueProps::MOVE),
            _ => VarClass::Calculated,
        };
        Self {
            name,
            base: Decimal::ZERO,
            upgrade: Decimal::ZERO,
            multiplier: Some(multiplier),
            class,
        }
    }

    /// The var with other `Props` (`new DamageVar(2m, ValueProp.Unpowered | ValueProp.Move)`).
    ///
    /// # Panics
    /// For a var whose class has no props.
    #[must_use]
    pub const fn props(self, props: ValueProps) -> Self {
        let class = match self.class {
            VarClass::Damage(_) => VarClass::Damage(props),
            VarClass::Block(_) => VarClass::Block(props),
            VarClass::OstyDamage(_) => VarClass::OstyDamage(props),
            VarClass::CalculatedDamage { osty, .. } => VarClass::CalculatedDamage { props, osty },
            VarClass::CalculatedBlock(_) => VarClass::CalculatedBlock(props),
            VarClass::Plain | VarClass::ExtraDamage | VarClass::Calculated => {
                panic!("a var without props")
            }
        };
        Self { class, ..self }
    }

    /// A named `BlockVar` (`new BlockVar("BlockNextTurn", 5m, ValueProp.Move)`).
    #[must_use]
    pub const fn block(self) -> Self {
        Self {
            class: VarClass::Block(ValueProps::MOVE),
            ..self
        }
    }

    /// `CalculatedDamageVar.FromOsty()`: Osty deals the damage.
    ///
    /// # Panics
    /// For a var that is not a `CalculatedDamageVar`.
    #[must_use]
    pub const fn from_osty(self) -> Self {
        let VarClass::CalculatedDamage { props, .. } = self.class else {
            panic!("only a CalculatedDamageVar comes from Osty")
        };
        Self {
            class: VarClass::CalculatedDamage { props, osty: true },
            ..self
        }
    }

    /// The var with `OnUpgrade`'s `UpgradeValueBy(by)`.
    #[must_use]
    pub const fn upgrade(self, by: i32) -> Self {
        Self {
            upgrade: Decimal::from_i32(by),
            ..self
        }
    }

    /// The canonical value at `upgrade_level`.
    fn at(self, upgrade_level: u8) -> Decimal {
        if upgrade_level > 0 {
            self.base + self.upgrade
        } else {
            self.base
        }
    }
}

impl CardKind {
    /// The card's declared vars (`CanonicalVars`, in order).
    #[must_use]
    pub fn vars(self) -> &'static [CardVar] {
        with_model!(self, m => m.vars())
    }

    /// `DynamicVars[name].BaseValue` of a fresh card at `upgrade_level` (no instance changes),
    /// if the card has the var. A calculated var's is its `CalculationBase`.
    #[must_use]
    pub fn base_var(self, name: Var, upgrade_level: u8) -> Option<Decimal> {
        let var = self.vars().iter().find(|v| v.name == name)?;
        Some(if var.multiplier.is_some() {
            self.base_var(Var::CalculationBase, upgrade_level)?
        } else {
            var.at(upgrade_level)
        })
    }
}

impl Card {
    /// `DynamicVars[name].BaseValue` of this instance, if the card has the var
    /// (`DynamicVars.ContainsKey(name)`).
    #[must_use]
    pub fn try_var(&self, name: Var) -> Option<Decimal> {
        let value = self.kind.base_var(name, self.upgrade_level)?;
        if let Some(stale) = self.stale_calculated.map(PackedDecimal::get)
            && self
                .kind
                .vars()
                .iter()
                .any(|v| v.name == name && v.multiplier.is_some())
        {
            return Some(stale);
        }
        Some(with_model!(self.kind, m => m.instance_var(self, name, value)))
    }

    /// `DynamicVars[name].BaseValue`.
    ///
    /// # Panics
    /// If the card has no such var (a `KeyNotFoundException` in the game).
    #[must_use]
    pub fn var(&self, name: Var) -> Decimal {
        self.try_var(name)
            .unwrap_or_else(|| panic!("{:?} has no {name} var", self.kind))
    }

    /// `DynamicVars[name].IntValue`: the base value truncated.
    ///
    /// # Panics
    /// If the card has no such var.
    #[must_use]
    pub fn int_var(&self, name: Var) -> i32 {
        self.var(name).to_int_truncating()
    }

    /// [`Card::int_var`] of a count (cards, hits, replays).
    ///
    /// # Panics
    /// If the card has no such var or it is negative.
    #[must_use]
    pub fn count_var(&self, name: Var) -> u32 {
        u32::try_from(self.int_var(name)).expect("a count var is not negative")
    }

    /// [`Card::count_var`] as a length (a number of cards to take).
    ///
    /// # Panics
    /// If the var is negative.
    #[must_use]
    pub fn count_var_len(&self, name: Var) -> usize {
        usize::try_from(self.int_var(name)).expect("a count var is not negative")
    }

    /// `CalculationBase.BaseValue`, if the card has a calculated var (every one reads it).
    #[must_use]
    pub fn calculation_base(&self) -> Option<Decimal> {
        self.kind
            .vars()
            .iter()
            .any(|v| v.multiplier.is_some())
            .then(|| self.try_var(Var::CalculationBase))
            .flatten()
    }

    /// `DowngradeInternal`'s var clone, after the downgrade: a calculated var keeps `old`, the
    /// `CalculationBase` before it ([`Card::stale_calculated`]).
    pub(crate) fn keep_stale_calculated(&mut self, old: Option<Decimal>) {
        let new = self.calculation_base();
        self.stale_calculated = old.filter(|&v| Some(v) != new).map(PackedDecimal::pack);
    }

    /// Every var of this instance with its base value, in declaration order.
    pub fn vars(&self) -> impl Iterator<Item = (Var, Decimal)> + '_ {
        self.kind.vars().iter().map(|v| (v.name, self.var(v.name)))
    }
}

impl CombatState {
    /// `CalculatedVar.Calculate(target)` of `card`'s calculated var `name`:
    /// `CalculationBase + extra * multiplier(card, target)` (the multiplier is 0 once the combat
    /// is no longer in progress, `CalculatedVar.cs:62-70`), if the card has that calculated var.
    #[must_use]
    pub fn calculate(
        &self,
        card: CardId,
        name: Var,
        target: Option<CreatureId>,
    ) -> Option<Decimal> {
        let c = self.card(card);
        let multiplier = c.kind.vars().iter().find(|v| v.name == name)?.multiplier?;
        let base = c.var(Var::CalculationBase);
        let extra = c.var(name.extra());
        let multiplier = if self.in_progress {
            multiplier(self, card, target)
        } else {
            Decimal::ZERO
        };
        Some(base + extra * multiplier)
    }

    /// [`CombatState::calculate`] truncated to an int (a calculated hit or card count).
    ///
    /// # Panics
    /// If the card has no such calculated var.
    #[must_use]
    pub fn calculate_int(&self, card: CardId, name: Var, target: Option<CreatureId>) -> i32 {
        self.calculate(card, name, target)
            .unwrap_or_else(|| panic!("{:?} has no {name} var", self.card(card).kind))
            .to_int_truncating()
    }
}

/// `CardPreviewMode` of a card's preview (`Upgrade` is the upgrade screen's, `None` the
/// engine's own: no preview).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PreviewMode {
    /// `Normal`: the card as the hand shows it.
    #[default]
    Normal,
    /// `MultiCreatureTargeting`: an `AllEnemies` or `RandomEnemy` card while it is played; the
    /// damage previews a modifier only if it is the same against every hittable enemy.
    MultiCreatureTargeting,
}

/// The enchantment part of `DamageVar`/`OstyDamageVar.UpdateCardPreview`: additive, then
/// multiplicative.
fn enchant_damage(enchantment: Option<Enchantment>, value: Decimal, props: ValueProps) -> Decimal {
    let Some(enchantment) = enchantment else {
        return value;
    };
    let m = enchantment.kind.model();
    let value = value + m.enchant_damage_additive(enchantment, value, props);
    value * m.enchant_damage_multiplicative(enchantment, value, props)
}

/// The enchantment part of `BlockVar.UpdateCardPreview`.
fn enchant_block(enchantment: Option<Enchantment>, value: Decimal) -> Decimal {
    let Some(enchantment) = enchantment else {
        return value;
    };
    let m = enchantment.kind.model();
    let value = value + m.enchant_block_additive(enchantment, value);
    value * m.enchant_block_multiplicative(enchantment, value)
}

/// `UpdateCardPreview` without the global hooks (`runGlobalHooks` false): `base` is the var's
/// `BaseValue`, `calculated` its `Calculate(target)` for a calculated var.
fn unhooked_preview(
    class: VarClass,
    base: Decimal,
    calculated: Decimal,
    enchantment: Option<Enchantment>,
) -> Decimal {
    match class {
        VarClass::Plain => base,
        VarClass::Calculated => calculated,
        VarClass::Damage(props) | VarClass::OstyDamage(props) => {
            enchant_damage(enchantment, base, props)
        }
        VarClass::Block(_) => enchant_block(enchantment, base),
        VarClass::ExtraDamage => enchantment.map_or(base, |e| {
            base * e
                .kind
                .model()
                .enchant_damage_multiplicative(e, base, ValueProps::MOVE)
        }),
        VarClass::CalculatedDamage { props, .. } => {
            enchant_damage(enchantment, calculated, props).max(Decimal::ZERO)
        }
        VarClass::CalculatedBlock(_) => enchant_block(enchantment, calculated),
    }
}

/// An enchantment's own `DynamicVars` (`CanonicalVars` after `RecalculateValues`): name, class
/// and `BaseValue`. Only these five declare vars; the rest show `Amount` alone.
fn enchantment_vars(enchantment: Enchantment) -> Vec<(&'static str, VarClass, Decimal)> {
    match enchantment.kind {
        // `BlockVar(0, Move)`, `RecalculateValues` sets it to `Amount`.
        EnchantmentKind::Adroit => vec![(
            "Block",
            VarClass::Block(ValueProps::MOVE),
            Decimal::from(enchantment.amount),
        )],
        EnchantmentKind::TezcatarasEmber => vec![(
            "Damage",
            VarClass::Damage(ValueProps::MOVE),
            tezcataras_ember::DAMAGE,
        )],
        // `PowerVar<WeakPower>(1)`.
        EnchantmentKind::Inky => vec![("WeakPower", VarClass::Plain, Decimal::ONE)],
        // `IntVar("Times", 1)`, `DynamicVar("Times", 1)`.
        EnchantmentKind::Spiral | EnchantmentKind::Glam => {
            vec![("Times", VarClass::Plain, Decimal::ONE)]
        }
        _ => Vec::new(),
    }
}

impl Card {
    /// `UpdateDynamicVarPreview(Normal, null, Enchantment.DynamicVars)` outside a fight
    /// (`NCard.UpdateVisuals`): the enchantment's vars previewed as the card's own, so its
    /// enchantment changes them too (Tezcatara's Ember's `Damage` shows 6).
    #[must_use]
    pub fn enchantment_preview_out_of_combat(&self) -> Vec<(&'static str, Decimal)> {
        self.enchantment.map_or_else(Vec::new, |e| {
            enchantment_vars(e)
                .into_iter()
                .map(|(name, class, base)| (name, unhooked_preview(class, base, base, Some(e))))
                .collect()
        })
    }

    /// `UpdateDynamicVarPreview` of a card outside a fight (`CardPreviewMode.Normal`, no
    /// target, no global hooks): every var with its `PreviewValue`, in declaration order. A
    /// calculated var's `Calculate(null)` is its `CalculationBase`; the enchantment's changes
    /// to damage and block show.
    pub fn preview_out_of_combat(&self) -> impl Iterator<Item = (Var, Decimal)> + '_ {
        self.kind.vars().iter().map(|v| {
            let base = self.var(v.name);
            let calculated = if v.multiplier.is_some() {
                self.var(Var::CalculationBase)
            } else {
                base
            };
            (
                v.name,
                unhooked_preview(v.class, base, calculated, self.enchantment),
            )
        })
    }
}

impl CombatState {
    /// `CardModel.UpdateDynamicVarPreview(mode, target, DynamicVars)` (`CardModel.cs:1452`) of
    /// a card of this fight: every var with its `PreviewValue` (the number the card's text
    /// shows), in declaration order.
    ///
    /// A card in the hand (or being played) runs the global hooks, so Strength, Vulnerable on
    /// `target`, Dexterity and the like count (`DamageVar`, `BlockVar`, `OstyDamageVar` and the
    /// calculated damage and block vars, `DamageVar.cs:28`); a card in another pile shows its
    /// values with only its enchantment's changes. `target` is the creature the card is aimed
    /// at (`None` while it is not aimed).
    ///
    /// `MultiCreatureTargeting` compares the damage against each hittable enemy after `max(0, x)`
    /// (the game compares before it; only a negative modified damage can tell them apart).
    #[must_use]
    pub fn card_preview(
        &self,
        card: CardId,
        target: Option<CreatureId>,
        mode: PreviewMode,
    ) -> Vec<(Var, Decimal)> {
        let c = self.card(card);
        let vars = c.kind.vars().iter().map(|v| {
            let base = c.var(v.name);
            let calculated = self.calculate(card, v.name, target).unwrap_or(base);
            (v.name, v.class, base, calculated)
        });
        self.preview_vars(card, target, mode, vars.collect())
    }

    /// `UpdateDynamicVarPreview(mode, target, Enchantment.DynamicVars)` (`NCard.UpdateVisuals`)
    /// of a card of this fight: its enchantment's vars previewed as the card's own vars are
    /// ([`CombatState::card_preview`]), so in the hand Adroit's `Block` counts Dexterity.
    #[must_use]
    pub fn enchantment_preview(
        &self,
        card: CardId,
        target: Option<CreatureId>,
        mode: PreviewMode,
    ) -> Vec<(&'static str, Decimal)> {
        let Some(enchantment) = self.card(card).enchantment else {
            return Vec::new();
        };
        let vars = enchantment_vars(enchantment)
            .into_iter()
            .map(|(name, class, base)| (name, class, base, base))
            .collect();
        self.preview_vars(card, target, mode, vars)
    }

    /// The `PreviewValue` of each of `vars` (name, class, `BaseValue`, calculated value) on
    /// `card`.
    fn preview_vars<N>(
        &self,
        card: CardId,
        target: Option<CreatureId>,
        mode: PreviewMode,
        vars: Vec<(N, VarClass, Decimal, Decimal)>,
    ) -> Vec<(N, Decimal)> {
        let c = self.card(card);
        let in_hand = matches!(self.pile_of(card), Some(PileType::Hand | PileType::Play));
        let player = self.player.creature;
        let multi = in_hand
            && target.is_none()
            && mode == PreviewMode::MultiCreatureTargeting
            && matches!(
                self.target_type(card),
                TargetType::AllEnemies | TargetType::RandomEnemy
            );
        let enemies = if multi {
            self.hittable_enemies()
        } else {
            Creatures::new()
        };
        {
            let h = self.hooked();
            // `Hook.ModifyDamage` with the preview mode (`Hook.cs:1495`).
            let damage = |dealer: Option<CreatureId>, props: ValueProps, amount: Decimal| {
                let modify = |target: Option<CreatureId>| {
                    let query = DamageQuery {
                        target,
                        dealer,
                        props,
                        card: Some(card),
                    };
                    h.modify_damage(query, amount).0
                };
                let mut same = None;
                for &enemy in &enemies {
                    let value = modify(Some(enemy));
                    match same {
                        None => same = Some(Some(value)),
                        Some(Some(first))
                            if first.to_int_truncating() != value.to_int_truncating() =>
                        {
                            same = Some(None);
                            break;
                        }
                        Some(_) => {}
                    }
                }
                same.flatten().unwrap_or_else(|| modify(target))
            };
            let block = |props: ValueProps, amount: Decimal| {
                let query = BlockQuery {
                    target: player,
                    props,
                    card: Some(card),
                    play: None,
                };
                h.modify_block(query, amount).0
            };
            vars.into_iter()
                .map(|(name, class, base, calculated)| {
                    let value = if in_hand {
                        match class {
                            VarClass::Damage(props) => damage(Some(player), props, base),
                            VarClass::OstyDamage(props) => damage(self.osty(), props, base),
                            VarClass::Block(props) => block(props, base),
                            VarClass::CalculatedDamage { props, osty } => {
                                let dealer = if osty { self.osty() } else { Some(player) };
                                damage(dealer, props, calculated).max(Decimal::ZERO)
                            }
                            VarClass::CalculatedBlock(props) => block(props, calculated),
                            class => unhooked_preview(class, base, calculated, c.enchantment),
                        }
                    } else {
                        unhooked_preview(class, base, calculated, c.enchantment)
                    };
                    (name, value)
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_the_game() {
        assert_eq!(Var::Damage.to_string(), "Damage");
        assert_eq!(Var::CalculatedDamage.to_string(), "CalculatedDamage");
        assert_eq!(
            Var::Power(PowerKind::Vulnerable).to_string(),
            "VulnerablePower"
        );
        assert_eq!(Var::Named("Increase").to_string(), "Increase");
    }

    #[test]
    fn base_values_follow_the_upgrade_level() {
        let strike = CardKind::StrikeIronclad;
        assert_eq!(strike.base_var(Var::Damage, 0), Some(Decimal::from(6)));
        assert_eq!(strike.base_var(Var::Damage, 1), Some(Decimal::from(9)));
        assert_eq!(strike.base_var(Var::Block, 0), None);
        let perfected = CardKind::PerfectedStrike;
        assert_eq!(
            perfected.base_var(Var::CalculatedDamage, 1),
            Some(Decimal::from(6))
        );
        assert_eq!(
            perfected.base_var(Var::ExtraDamage, 1),
            Some(Decimal::from(3))
        );
    }

    /// `Thrash.AfterDowngraded` re-adds `ExtraDamage` to the canonical damage.
    #[test]
    fn instance_growth_survives_a_downgrade() {
        use crate::combat::CardId;
        use crate::content::cards::{CardData, thrash};
        let mut card = Card::new(CardId(0), CardKind::Thrash, 1);
        card.data = CardData::Thrash(thrash::Data {
            extra_damage: Decimal::from(7).into(),
        });
        assert_eq!(card.var(Var::Damage), Decimal::from(13));
        card.upgrade_level = 0;
        assert_eq!(card.var(Var::Damage), Decimal::from(11));
        assert_eq!(card.try_var(Var::Block), None);
    }

    /// `DynamicVarSet` rejects duplicate keys, and a calculated var reads its base and extra
    /// vars.
    #[test]
    fn every_card_declares_a_valid_var_set() {
        for kind in CardKind::ALL.iter().copied() {
            let vars = kind.vars();
            for (i, v) in vars.iter().enumerate() {
                assert!(
                    vars[..i].iter().all(|w| w.name != v.name),
                    "{kind:?} declares {} twice",
                    v.name
                );
                if v.multiplier.is_some() {
                    for needed in [Var::CalculationBase, v.name.extra()] {
                        assert!(
                            vars.iter().any(|w| w.name == needed),
                            "{kind:?}'s {} needs {needed}",
                            v.name
                        );
                    }
                }
            }
        }
    }
}
