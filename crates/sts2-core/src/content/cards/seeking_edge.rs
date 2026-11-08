//! `SeekingEdge` (`Models.Cards/SeekingEdge.cs`): Regent rare power, cost 1; apply 1
//! `SeekingEdgePower`, then Forge 7 (11 upgraded).

use crate::combat::hooks::{Hooks, Listener};
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct SeekingEdge;

#[sts2_macros::hooks]
impl Hooks for SeekingEdge {
    type Me = CardId;
}

impl CardModel for SeekingEdge {
    fn name(&self) -> &'static str {
        "Seeking Edge"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Forge, 7).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::SeekingEdge,
            Decimal::ONE,
            Some(player),
        )?;
        let forge = eng.s.card(play.card).var(Var::Forge);
        eng.forge(forge, Some(Listener::Card(play.card)))?;
        Ok(())
    }
}
