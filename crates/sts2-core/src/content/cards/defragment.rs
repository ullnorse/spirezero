//! `Defragment` (`Models.Cards/Defragment.cs`): cost 1 power; 1 Focus (2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Defragment;

#[sts2_macros::hooks]
impl Hooks for Defragment {
    type Me = CardId;
}

impl CardModel for Defragment {
    fn name(&self) -> &'static str {
        "Defragment"
    }

    fn cost(&self) -> i32 {
        1
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Focus), 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let focus = eng.s.card(play.card).int_var(Var::Power(PowerKind::Focus));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Focus,
            Decimal::from(focus),
            Some(player),
        )
    }
}
