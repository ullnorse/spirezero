//! `BorrowedTime` (`Models.Cards/BorrowedTime.cs`): cost 1; gain 4 Energy (6 upgraded), then every card costs 1 more until the end of the turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct BorrowedTime;

#[sts2_macros::hooks]
impl Hooks for BorrowedTime {
    type Me = CardId;
}

impl CardModel for BorrowedTime {
    fn name(&self) -> &'static str {
        "Borrowed Time"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Energy, 4).upgrade(2),
            CardVar::new(Var::Named("ExtraCost"), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.gain_energy(eng.s.card(play.card).var(Var::Energy))?;
        let player = eng.s.player.creature;
        let extra_cost = eng.s.card(play.card).var(Var::Named("ExtraCost"));
        eng.apply_power(player, PowerKind::BorrowedTime, extra_cost, Some(player))
    }
}
