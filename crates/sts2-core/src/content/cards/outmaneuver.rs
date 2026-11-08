//! `Outmaneuver` (`Models.Cards/Outmaneuver.cs`): Event skill, cost 1; 2 energy (3 upgraded) next
//! turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Outmaneuver;

#[sts2_macros::hooks]
impl Hooks for Outmaneuver {
    type Me = CardId;
}

impl CardModel for Outmaneuver {
    fn name(&self) -> &'static str {
        "Outmaneuver"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let energy = eng.s.card(play.card).var(Var::Energy);
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::EnergyNextTurn,
            energy,
            Some(player),
        )
    }
}
