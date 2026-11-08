//! `Royalties` (`Models.Cards/Royalties.cs`): Regent rare power, cost 1, never generated in
//! combat; apply 30 `RoyaltiesPower` (40 upgraded), extra gold when the combat is won.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Royalties;

#[sts2_macros::hooks]
impl Hooks for Royalties {
    type Me = CardId;
}

impl CardModel for Royalties {
    fn name(&self) -> &'static str {
        "Royalties"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Gold, 30).upgrade(10)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Gold);
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Royalties,
            amount,
            Some(player),
        )
    }
}
