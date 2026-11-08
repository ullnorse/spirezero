//! `Poke` (`Models.Cards/Poke.cs`): cost 0, Osty hits for 6 (9 upgraded). Does nothing without a
//! living Osty.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Poke;

#[sts2_macros::hooks]
impl Hooks for Poke {
    type Me = CardId;
}

impl CardModel for Poke {
    fn name(&self) -> &'static str {
        "Poke"
    }

    fn cost(&self) -> i32 {
        0
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

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::OstyDamage, 6).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Poke needs a target");
        // Osty.CheckMissingWithAnim.
        if !eng.s.is_osty_alive() {
            return Ok(());
        }
        let damage = eng.s.card(play.card).var(Var::OstyDamage);
        let osty = eng.s.osty().expect("Osty is alive");
        eng.attack(&Attack::new(
            osty,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
