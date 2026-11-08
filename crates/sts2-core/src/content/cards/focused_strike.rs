//! `FocusedStrike` (`Models.Cards/FocusedStrike.cs`): cost 1 Strike; 9 damage (11 upgraded), then
//! 1 Focus (2 upgraded) until the end of the turn (`FocusedStrikePower`).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct FocusedStrike;

#[sts2_macros::hooks]
impl Hooks for FocusedStrike {
    type Me = CardId;
}

impl CardModel for FocusedStrike {
    fn name(&self) -> &'static str {
        "Focused Strike"
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
            CardVar::new(Var::Damage, 9).upgrade(2),
            CardVar::new(Var::Power(PowerKind::Focus), 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("FocusedStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let focus = eng.s.card(play.card).int_var(Var::Power(PowerKind::Focus));
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
            PowerKind::FocusedStrike,
            Decimal::from(focus),
            Some(player),
        )
    }
}
