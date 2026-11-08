//! `CelestialMight` (`Models.Cards/CelestialMight.cs`): Regent common attack, cost 2; deal 6 damage 3 times (4 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct CelestialMight;

#[sts2_macros::hooks]
impl Hooks for CelestialMight {
    type Me = CardId;
}

impl CardModel for CelestialMight {
    fn name(&self) -> &'static str {
        "Celestial Might"
    }

    fn cost(&self) -> i32 {
        2
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
            CardVar::new(Var::Damage, 6),
            CardVar::new(Var::Repeat, 3).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("CelestialMight needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(eng.s.card(play.card).count_var(Var::Repeat)),
        )?;
        Ok(())
    }
}
