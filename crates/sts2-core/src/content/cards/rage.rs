//! `Rage` (`Models.Cards/Rage.cs`): cost 0, gain 3 Block (5 upgraded) per Attack played this turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Rage;

#[sts2_macros::hooks]
impl Hooks for Rage {
    type Me = CardId;
}

impl CardModel for Rage {
    fn name(&self) -> &'static str {
        "Rage"
    }

    fn cost(&self) -> i32 {
        0
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Power"), 3).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Rage,
            eng.s.card(play.card).var(Var::Named("Power")),
            Some(player),
        )
    }
}
