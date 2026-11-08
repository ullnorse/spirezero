//! `SetupStrike` (`Models.Cards/SetupStrike.cs`): cost 1, 7 damage (9 upgraded), then 3 Strength (4 upgraded) this turn.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct SetupStrike;

#[sts2_macros::hooks]
impl Hooks for SetupStrike {
    type Me = CardId;
}

impl CardModel for SetupStrike {
    fn name(&self) -> &'static str {
        "Setup Strike"
    }

    fn cost(&self) -> i32 {
        1
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

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 7).upgrade(2),
            CardVar::new(Var::Power(PowerKind::Strength), 3).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("SetupStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::SetupStrike,
            eng.s.card(play.card).var(Var::Power(PowerKind::Strength)),
            Some(player),
        )
    }
}
