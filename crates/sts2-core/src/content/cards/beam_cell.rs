//! `BeamCell` (`Models.Cards/BeamCell.cs`): cost 0, 3 damage (4 upgraded), then 1 Vulnerable (2 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct BeamCell;

#[sts2_macros::hooks]
impl Hooks for BeamCell {
    type Me = CardId;
}

impl CardModel for BeamCell {
    fn name(&self) -> &'static str {
        "Beam Cell"
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
            CardVar::new(Var::Power(PowerKind::Vulnerable), 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("BeamCell needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Vulnerable));
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Vulnerable,
            amount,
            Some(player),
        )
    }
}
