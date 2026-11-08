//! `SweepingGaze` (`Models.Cards/SweepingGaze.cs`): token, cost 0, Ethereal and Exhaust; Osty
//! hits a random enemy for 10 (15 upgraded). Does nothing without a living Osty.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct SweepingGaze;

#[sts2_macros::hooks]
impl Hooks for SweepingGaze {
    type Me = CardId;
}

impl CardModel for SweepingGaze {
    fn name(&self) -> &'static str {
        "Sweeping Gaze"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Token
    }

    fn target(&self) -> TargetType {
        TargetType::RandomEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE
            .with(CardKeyword::Ethereal)
            .with(CardKeyword::Exhaust)
    }

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::OstyDamage, 10).upgrade(5)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // Osty.CheckMissingWithAnim.
        if !eng.s.is_osty_alive() {
            return Ok(());
        }
        let damage = eng.s.card(play.card).var(Var::OstyDamage);
        let osty = eng.s.osty().expect("Osty is alive");
        eng.attack(&Attack::new(
            osty,
            AttackTargets::RandomOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
