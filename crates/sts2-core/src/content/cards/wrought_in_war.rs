//! `WroughtInWar` (`Models.Cards/WroughtInWar.cs`): Regent common attack, cost 1; deal 7 damage
//! (9 upgraded), then Forge 7 (9 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::{Hooks, Listener};
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct WroughtInWar;

#[sts2_macros::hooks]
impl Hooks for WroughtInWar {
    type Me = CardId;
}

impl CardModel for WroughtInWar {
    fn name(&self) -> &'static str {
        "Wrought in War"
    }

    fn cost(&self) -> i32 {
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
            CardVar::new(Var::Damage, 7).upgrade(2),
            CardVar::new(Var::Forge, 7).upgrade(2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("WroughtInWar needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let forge = Decimal::from(eng.s.card(play.card).int_var(Var::Forge));
        eng.forge(forge, Some(Listener::Card(play.card)))?;
        Ok(())
    }
}
