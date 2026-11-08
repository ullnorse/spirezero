//! `Hyperbeam` (`Models.Cards/Hyperbeam.cs`): cost 2 Rare Attack; 24 damage (30 upgraded) to all
//! enemies, then 3 Focus down until the end of the turn (`HyperbeamFocusDownPower`). The beam
//! VFX in `BeforeDamage` is presentation.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Hyperbeam;

#[sts2_macros::hooks]
impl Hooks for Hyperbeam {
    type Me = CardId;
}

impl CardModel for Hyperbeam {
    fn name(&self) -> &'static str {
        "Hyperbeam"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 24).upgrade(6),
            CardVar::new(Var::Power(PowerKind::Focus), 3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let focus = eng.s.card(play.card).int_var(Var::Power(PowerKind::Focus));
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::HyperbeamFocusDown,
            Decimal::from(focus),
            Some(player),
        )
    }
}
