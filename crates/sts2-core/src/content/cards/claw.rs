//! `Claw` (`Models.Cards/Claw.cs`): cost 0, 3 damage (4 upgraded). After the hit every Claw of the combat (all piles,
//! `PlayerCombatState.AllCards`) gains `Increase` (2, 3 upgraded) damage for the rest of the combat. The game adds the gain to
//! the `DamageVar`'s base value and to `_extraDamageFromClawPlays`, and `AfterDowngraded` (which resets the vars) adds the
//! latter back. The card keeps only the sum of the gains, so its damage is canonical + extra before and after a downgrade.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardData, CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct Claw;

#[sts2_macros::hooks]
impl Hooks for Claw {
    type Me = CardId;
}

impl CardModel for Claw {
    fn name(&self) -> &'static str {
        "Claw"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 3).upgrade(1),
            CardVar::new(Var::Named("Increase"), 2).upgrade(1),
        ];
        VARS
    }

    fn instance_var(&self, card: &Card, name: Var, value: Decimal) -> Decimal {
        match name {
            Var::Damage => value + Decimal::from(extra_damage(card)),
            _ => value,
        }
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Claw needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let increase = eng.s.card(play.card).int_var(Var::Named("Increase"));
        let claws: Vec<CardId> = PileType::LISTENER_ORDER
            .into_iter()
            .flat_map(|pile| eng.s.player.pile(pile).iter().copied())
            .filter(|&c| eng.s.card(c).kind == CardKind::Claw)
            .collect();
        for claw in claws {
            let extra = extra_damage(eng.s.card(claw));
            eng.s.card_mut(claw).data = CardData::Claw(Data {
                extra_damage_from_claw_plays: extra + increase,
            });
        }
        Ok(())
    }
}

/// `_extraDamageFromClawPlays`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub extra_damage_from_claw_plays: i32,
}

const fn extra_damage(card: &Card) -> i32 {
    match card.data {
        CardData::Claw(data) => data.extra_damage_from_claw_plays,
        _ => 0,
    }
}
