//! `Salvo` (`Models.Cards/Salvo.cs`): cost 1; 12 damage (16 upgraded), then the hand is
//! retained this turn (`RetainHandPower`). It has no `CanonicalKeywords` (the Retain in the inventory
//! is a hover tip, like `Eidolon`).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Salvo;

#[sts2_macros::hooks]
impl Hooks for Salvo {
    type Me = CardId;
}

impl CardModel for Salvo {
    fn name(&self) -> &'static str {
        "Salvo"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 12).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Salvo needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.apply_power(player, PowerKind::RetainHand, Decimal::ONE, Some(player))
    }
}
