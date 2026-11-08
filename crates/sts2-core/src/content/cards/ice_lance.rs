//! `IceLance` (`Models.Cards/IceLance.cs`): cost 3, 19 damage (24 upgraded), then channel 3 Frost.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct IceLance;

#[sts2_macros::hooks]
impl Hooks for IceLance {
    type Me = CardId;
}

impl CardModel for IceLance {
    fn name(&self) -> &'static str {
        "Ice Lance"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 19).upgrade(5),
            CardVar::new(Var::Repeat, 3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("IceLance needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        for _ in 0..eng.s.card(play.card).count_var(Var::Repeat) {
            eng.channel(OrbKind::Frost)?;
        }
        Ok(())
    }
}
