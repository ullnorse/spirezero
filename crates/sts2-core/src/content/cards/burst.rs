//! `Burst` (`Models.Cards/Burst.cs`): cost 1, the next Skill (2 upgraded) is played twice.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Burst;

#[sts2_macros::hooks]
impl Hooks for Burst {
    type Me = CardId;
}

impl CardModel for Burst {
    fn name(&self) -> &'static str {
        "Burst"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Skills"), 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Burst,
            eng.s.card(play.card).var(Var::Named("Skills")),
            Some(player),
        )
    }
}
