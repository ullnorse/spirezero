//! `SicEm` (`Models.Cards/SicEm.cs`): cost 1, Osty hits for 5 (6 upgraded) if alive, then the
//! target gets `SicEmPower` 3 (4 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct SicEm;

#[sts2_macros::hooks]
impl Hooks for SicEm {
    type Me = CardId;
}

impl CardModel for SicEm {
    fn name(&self) -> &'static str {
        "Sic 'Em"
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

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::OstyDamage, 5).upgrade(1),
            CardVar::new(Var::Power(PowerKind::SicEm), 3).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("SicEm needs a target");
        // Osty.CheckMissingWithAnim.
        if eng.s.is_osty_alive() {
            let damage = eng.s.card(play.card).var(Var::OstyDamage);
            let osty = eng.s.osty().expect("Osty is alive");
            eng.attack(&Attack::new(
                osty,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            ))?;
        }
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::SicEm));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::SicEm,
            amount,
            Some(player),
        )
    }
}
