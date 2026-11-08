//! `GuidingStar` (`Models.Cards/GuidingStar.cs`): Regent common attack, cost 1 and 1 star; deal 12 damage (13 upgraded), then draw 2 cards (3 upgraded) next turn.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct GuidingStar;

#[sts2_macros::hooks]
impl Hooks for GuidingStar {
    type Me = CardId;
}

impl CardModel for GuidingStar {
    fn name(&self) -> &'static str {
        "Guiding Star"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn star_cost(&self) -> i32 {
        1
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
            CardVar::new(Var::Damage, 12).upgrade(1),
            CardVar::new(Var::Cards, 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("GuidingStar needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::DrawCardsNextTurn,
            eng.s.card(play.card).var(Var::Cards),
            Some(player),
        )?;
        Ok(())
    }
}
