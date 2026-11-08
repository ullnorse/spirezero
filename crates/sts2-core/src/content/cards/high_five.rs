//! `HighFive` (`Models.Cards/HighFive.cs`): cost 2, only playable with a living Osty; Osty hits
//! every enemy for 11 (13 upgraded), then they get 2 (3) Vulnerable.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct HighFive;

#[sts2_macros::hooks]
impl Hooks for HighFive {
    type Me = CardId;
}

impl CardModel for HighFive {
    fn name(&self) -> &'static str {
        "High Five"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn is_playable(&self, s: &CombatState, _me: CardId) -> bool {
        s.is_osty_alive()
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::OstyDamage, 11).upgrade(2),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // Osty.CheckMissingWithAnim.
        if !eng.s.is_osty_alive() {
            return Ok(());
        }
        let damage = eng.s.card(play.card).var(Var::OstyDamage);
        let vulnerable = eng.s.card(play.card).var(Var::Power(PowerKind::Vulnerable));
        let osty = eng.s.osty().expect("Osty is alive");
        eng.attack(&Attack::new(
            osty,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let player = eng.s.player.creature;
        let targets = eng.hooked().hittable_enemies();
        eng.apply_power_to_all_from_card(
            Some(play.card),
            &targets,
            PowerKind::Vulnerable,
            vulnerable,
            Some(player),
        )
    }
}
