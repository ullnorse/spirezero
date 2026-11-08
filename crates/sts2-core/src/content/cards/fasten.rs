//! `Fasten` (`Models.Cards/Fasten.cs`): cost 1 power; cards tagged `Defend` give 4 (6 upgraded)
//! more Block. The `Defend` tag is [`CardModel::is_defend`] (Fasten itself is not tagged; it only
//! reads the tag in its hover tip).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Fasten;

#[sts2_macros::hooks]
impl Hooks for Fasten {
    type Me = CardId;
}

impl CardModel for Fasten {
    fn name(&self) -> &'static str {
        "Fasten"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("ExtraBlock"), 4).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let extra = eng.s.card(play.card).var(Var::Named("ExtraBlock"));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Fasten, extra, Some(player))
    }
}
