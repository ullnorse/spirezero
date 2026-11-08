//! `Hegemony` (`Models.Cards/Hegemony.cs`): Regent uncommon attack, cost 2; deal 15 damage (18 upgraded), then gain 2 energy (3 upgraded) next turn.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Hegemony;

#[sts2_macros::hooks]
impl Hooks for Hegemony {
    type Me = CardId;
}

impl CardModel for Hegemony {
    fn name(&self) -> &'static str {
        "Hegemony"
    }

    fn cost(&self) -> i32 {
        2
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
            CardVar::new(Var::Damage, 15).upgrade(3),
            CardVar::new(Var::Energy, 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Hegemony needs a target");
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
            PowerKind::EnergyNextTurn,
            eng.s.card(play.card).var(Var::Energy),
            Some(player),
        )?;
        Ok(())
    }
}
