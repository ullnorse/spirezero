//! `SovereignBlade` (`Models.Cards/SovereignBlade.cs`): the Regent's Forge token. Attack, cost 2
//! (1 upgraded), Token rarity, Retain; deals its damage (10 plus every Forge) `Repeat` times.
//! With `SeekingEdgePower` on the owner it targets every enemy; with `ParryPower` it then
//! gains block equal to the Parry amount. The damage and repeats are the card's own fields
//! (`_currentDamage`, `_currentRepeats`), which survive a downgrade (`AfterDowngraded`).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{
    CardId, CardPlay, CombatState, CreatureId, Engine, Flow, PileType, ValueProps,
};
use crate::content::cards::{
    CardData, CardKeyword, CardKind, CardModel, CardRarity, CardType, CardVar, Keywords,
    TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::{Decimal, PackedDecimal};

pub struct SovereignBlade;

/// `SovereignBlade`'s fields, packed (they sit in every card's [`CardData`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `DynamicVars.Damage.BaseValue` (`CurrentDamage`).
    pub damage: PackedDecimal,
    /// `DynamicVars.Repeat.BaseValue` (`CurrentRepeats`): the hit count.
    pub repeats: PackedDecimal,
}

/// `_baseDamage`, `RepeatVar(1)`.
const DEFAULT: Data = Data {
    damage: PackedDecimal::pack(Decimal::new(10, 0)),
    repeats: PackedDecimal::pack(Decimal::ONE),
};

/// The blade's fields (a fresh blade's until forged).
#[must_use]
pub const fn data(card: &Card) -> Data {
    match card.data {
        CardData::SovereignBlade(data) => data,
        _ => DEFAULT,
    }
}

/// `AddDamage(amount)`: `Damage.BaseValue += amount`.
pub fn add_damage(card: &mut Card, amount: Decimal) {
    let mut d = data(card);
    d.damage = (d.damage.get() + amount).into();
    card.data = CardData::SovereignBlade(d);
}

/// The Sovereign Blades in `AllCards` (a card in no pile is not in it).
#[must_use]
pub fn all_blades(s: &CombatState) -> Vec<CardId> {
    PileType::LISTENER_ORDER
        .into_iter()
        .flat_map(|p| s.player.pile(p).iter().copied())
        .filter(|&c| s.card(c).kind == CardKind::SovereignBlade)
        .collect()
}

/// `HasSeekingEdge`: the owner has `SeekingEdgePower`.
fn has_seeking_edge(s: &CombatState) -> bool {
    s.creature(s.player.creature)
        .has_power(PowerKind::SeekingEdge)
}

/// `GetOwnerParryAmount`: the owner's `ParryPower` amount while the card is in a combat pile.
fn owner_parry_amount(s: &CombatState, me: CardId) -> i32 {
    if s.pile_of(me).is_none() {
        return 0;
    }
    s.creature(s.player.creature).power_amount(PowerKind::Parry)
}

/// `GetOwnerParryAmount(card)`.
fn parry_multiplier(s: &CombatState, card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(owner_parry_amount(s, card))
}

#[sts2_macros::hooks]
impl Hooks for SovereignBlade {
    type Me = CardId;
}

impl CardModel for SovereignBlade {
    fn name(&self) -> &'static str {
        "Sovereign Blade"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Token
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn target_in(&self, s: &CombatState, _me: CardId) -> TargetType {
        if has_seeking_edge(s) {
            TargetType::AllEnemies
        } else {
            TargetType::AnyEnemy
        }
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Retain)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 10),
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::CalculatedBlock, parry_multiplier),
            CardVar::new(Var::Repeat, 1),
        ];
        VARS
    }

    fn instance_var(&self, card: &Card, name: Var, value: Decimal) -> Decimal {
        match name {
            Var::Damage => data(card).damage.get(),
            Var::Repeat => data(card).repeats.get(),
            _ => value,
        }
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let targets = if has_seeking_edge(eng.s) {
            AttackTargets::AllOpponents
        } else {
            AttackTargets::Single(play.target.expect("SovereignBlade needs a target"))
        };
        let player = eng.s.player.creature;
        let card = eng.s.card(play.card);
        let damage = card.var(Var::Damage);
        let hits = u32::try_from(card.int_var(Var::Repeat)).unwrap_or(0);
        eng.attack(
            &Attack::new(
                player,
                targets,
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        let parry = owner_parry_amount(eng.s, play.card);
        if parry > 0 {
            let block = eng
                .s
                .calculate(play.card, Var::CalculatedBlock, play.target)
                .expect("SovereignBlade has a CalculatedBlock var");
            eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        }
        Ok(())
    }
}
