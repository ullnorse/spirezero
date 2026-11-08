//! `Sunder` (`Models.Cards/Sunder.cs`): Defect uncommon attack, cost 3; 26 damage (34 upgraded),
//! then gain 3 energy if any hit killed the target (`WasTargetKilled`: no Fatal/Minion check).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Sunder;

#[sts2_macros::hooks]
impl Hooks for Sunder {
    type Me = CardId;
}

impl CardModel for Sunder {
    fn name(&self) -> &'static str {
        "Sunder"
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
            CardVar::new(Var::Damage, 26).upgrade(8),
            CardVar::new(Var::Energy, 3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Sunder needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        let results = eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        if results.iter().any(|r| r.was_target_killed) {
            eng.gain_energy(Decimal::from(eng.s.card(play.card).int_var(Var::Energy)))?;
        }
        Ok(())
    }
}
