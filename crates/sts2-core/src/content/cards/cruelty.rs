//! `Cruelty` (`Models.Cards/Cruelty.cs`, an Ironclad card): cost 1 power; applies 25 Cruelty (50
//! upgraded). Not in any pool the Necrobinder sees: it is the only source of `CrueltyPower`, so
//! it exists to prove the power by a loadout (`c14-cruelty_power.json`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

/// An Ironclad card, kept to prove `CrueltyPower`.
pub struct Cruelty;

#[sts2_macros::hooks]
impl Hooks for Cruelty {
    type Me = CardId;
}

impl CardModel for Cruelty {
    fn name(&self) -> &'static str {
        "Cruelty"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Cruelty), 25).upgrade(25)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Cruelty));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Cruelty,
            amount,
            Some(player),
        )
    }
}
