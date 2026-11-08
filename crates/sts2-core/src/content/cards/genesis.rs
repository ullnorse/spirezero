//! `Genesis` (`Models.Cards/Genesis.cs`): Regent rare power, cost 2; gain 2 Genesis (3 upgraded): stars each turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Genesis;

#[sts2_macros::hooks]
impl Hooks for Genesis {
    type Me = CardId;
}

impl CardModel for Genesis {
    fn name(&self) -> &'static str {
        "Genesis"
    }

    fn cost(&self) -> i32 {
        2
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("StarsPerTurn"), 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Named("StarsPerTurn"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Genesis,
            amount,
            Some(player),
        )
    }
}
