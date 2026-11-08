//! `Maul` (`Models.Cards/Maul.cs`): Ancient (`Claws`); cost 1, 5 damage twice (6 upgraded). After
//! the hits every Maul of the combat (all piles, `PlayerCombatState.AllCards`) gains `Increase` (2,
//! 3 upgraded) damage for the rest of the combat. As `Claw`: the gain goes to the `DamageVar`'s base
//! value and to `_extraDamageFromMaulPlays`, which `AfterDowngraded` adds back.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardData, CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::decimal::Decimal;

/// `WithHitCount(2)`.
const HITS: u32 = 2;

pub struct Maul;

#[sts2_macros::hooks]
impl Hooks for Maul {
    type Me = CardId;
}

impl CardModel for Maul {
    fn name(&self) -> &'static str {
        "Maul"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 5).upgrade(1),
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
        let target = play.target.expect("Maul needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(HITS),
        )?;
        let increase = eng.s.card(play.card).int_var(Var::Named("Increase"));
        let mauls: Vec<CardId> = PileType::LISTENER_ORDER
            .into_iter()
            .flat_map(|pile| eng.s.player.pile(pile).iter().copied())
            .filter(|&c| eng.s.card(c).kind == CardKind::Maul)
            .collect();
        for maul in mauls {
            let extra = extra_damage(eng.s.card(maul));
            eng.s.card_mut(maul).data = CardData::Maul(Data {
                extra_damage_from_maul_plays: extra + increase,
            });
        }
        Ok(())
    }
}

/// `_extraDamageFromMaulPlays`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub extra_damage_from_maul_plays: i32,
}

const fn extra_damage(card: &Card) -> i32 {
    match card.data {
        CardData::Maul(data) => data.extra_damage_from_maul_plays,
        _ => 0,
    }
}
