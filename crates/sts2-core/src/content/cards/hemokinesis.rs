//! `Hemokinesis` (`Models.Cards/Hemokinesis.cs`): cost 1, loses 2 HP (unblockable, unpowered), then 15 damage (20 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Hemokinesis;

#[sts2_macros::hooks]
impl Hooks for Hemokinesis {
    type Me = CardId;
}

impl CardModel for Hemokinesis {
    fn name(&self) -> &'static str {
        "Hemokinesis"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::HpLoss, 2),
            CardVar::new(Var::Damage, 15).upgrade(5),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Hemokinesis needs a target");
        let player = eng.s.player.creature;
        eng.damage(
            &[player],
            eng.s.card(play.card).var(Var::HpLoss),
            ValueProps::UNBLOCKABLE
                .with(ValueProps::UNPOWERED)
                .with(ValueProps::MOVE),
            Some(player),
            Some(play.card),
        )?;
        let damage = eng.s.card(play.card).var(Var::Damage);
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
