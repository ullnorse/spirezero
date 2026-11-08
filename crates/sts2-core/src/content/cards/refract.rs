//! `Refract` (`Models.Cards/Refract.cs`): cost 3, 10 damage twice (13 upgraded), then channel 2 Glass.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Refract;

#[sts2_macros::hooks]
impl Hooks for Refract {
    type Me = CardId;
}

impl CardModel for Refract {
    fn name(&self) -> &'static str {
        "Refract"
    }

    fn cost(&self) -> i32 {
        3
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
            CardVar::new(Var::Repeat, 2),
            CardVar::new(Var::Damage, 10).upgrade(3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Refract needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(2),
        )?;
        for _ in 0..eng.s.card(play.card).count_var(Var::Repeat) {
            eng.channel(OrbKind::Glass)?;
        }
        Ok(())
    }
}
