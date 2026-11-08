//! `TeslaCoil` (`Models.Cards/TeslaCoil.cs`): cost 0, 3 damage (4 upgraded), then the passive of every Lightning orb on the target (twice upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Orb, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct TeslaCoil;

#[sts2_macros::hooks]
impl Hooks for TeslaCoil {
    type Me = CardId;
}

impl CardModel for TeslaCoil {
    fn name(&self) -> &'static str {
        "Tesla Coil"
    }

    fn cost(&self) -> i32 {
        0
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("TeslaCoil needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // The Lightning orbs are listed once; each gets its raw passive on the card's target
        // (twice upgraded), bypassing `ModifyOrbPassiveTriggerCount`.
        let upgraded = eng.s.card(play.card).is_upgraded();
        let lightning: Vec<Orb> = eng
            .s
            .player
            .orbs
            .iter()
            .copied()
            .filter(|o| o.kind == OrbKind::Lightning)
            .collect();
        for orb in lightning {
            eng.orb_passive(orb, Some(target), false)?;
            if upgraded {
                eng.orb_passive(orb, Some(target), false)?;
            }
        }
        Ok(())
    }
}
