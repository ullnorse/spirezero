//! `ByrdSwoop` (`Models.Cards/ByrdSwoop.cs`): cost 0, Event rarity, 14 damage (18 upgraded) to one
//! enemy. Hatching `ByrdonisEgg` turns the eggs into it; the attacker animation uses the Byrdpip
//! pet and is cosmetic.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct ByrdSwoop;

#[sts2_macros::hooks]
impl Hooks for ByrdSwoop {
    type Me = CardId;
}

impl CardModel for ByrdSwoop {
    fn name(&self) -> &'static str {
        "Byrd Swoop"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 14).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("ByrdSwoop needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
