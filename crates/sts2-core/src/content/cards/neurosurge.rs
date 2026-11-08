//! `Neurosurge` (`Models.Cards/Neurosurge.cs`): cost 0 power; gains 3 energy (4 upgraded), draws
//! 2 cards, then applies 3 Neurosurge.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Neurosurge;

#[sts2_macros::hooks]
impl Hooks for Neurosurge {
    type Me = CardId;
}

impl CardModel for Neurosurge {
    fn name(&self) -> &'static str {
        "Neurosurge"
    }

    fn cost(&self) -> i32 {
        0
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Neurosurge), 3),
            CardVar::new(Var::Energy, 3).upgrade(1),
            CardVar::new(Var::Cards, 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let energy = eng.s.card(play.card).int_var(Var::Energy);
        eng.gain_energy(Decimal::from(energy))?;
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        let player = eng.s.player.creature;
        eng.apply_power(
            player,
            PowerKind::Neurosurge,
            Decimal::from(
                eng.s
                    .card(play.card)
                    .int_var(Var::Power(PowerKind::Neurosurge)),
            ),
            Some(player),
        )
    }
}
