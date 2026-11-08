//! `Panache` (`Models.Cards/Panache.cs`): power, cost 0; apply a new `PanachePower` instance of 10
//! (14 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Panache;

#[sts2_macros::hooks]
impl Hooks for Panache {
    type Me = CardId;
}

impl CardModel for Panache {
    fn name(&self) -> &'static str {
        "Panache"
    }

    fn cost(&self) -> i32 {
        0
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("PanacheDamage"), 10).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Named("PanacheDamage"));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Panache, amount, Some(player))
    }
}
