//! `NeutronAegis` (`Models.Cards/NeutronAegis.cs`): Regent rare power, cost 1 and 5 stars; gain
//! 8 Plating (11 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct NeutronAegis;

#[sts2_macros::hooks]
impl Hooks for NeutronAegis {
    type Me = CardId;
}

impl CardModel for NeutronAegis {
    fn name(&self) -> &'static str {
        "Neutron Aegis"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn star_cost(&self) -> i32 {
        5
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Plating), 8).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let plating = eng.s.card(play.card).var(Var::Power(PowerKind::Plating));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Plating,
            plating,
            Some(player),
        )
    }
}
