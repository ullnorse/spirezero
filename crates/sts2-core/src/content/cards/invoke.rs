//! `Invoke` (`Models.Cards/Invoke.cs`): cost 1; next turn summon 2 (3 upgraded) and gain 2 (3)
//! energy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Invoke;

#[sts2_macros::hooks]
impl Hooks for Invoke {
    type Me = CardId;
}

impl CardModel for Invoke {
    fn name(&self) -> &'static str {
        "Invoke"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Summon, 2).upgrade(1),
            CardVar::new(Var::Energy, 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let summon = Decimal::from(eng.s.card(play.card).int_var(Var::Summon));
        let energy = Decimal::from(eng.s.card(play.card).int_var(Var::Energy));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::SummonNextTurn, summon, Some(player))?;
        eng.apply_power(player, PowerKind::EnergyNextTurn, energy, Some(player))
    }
}
