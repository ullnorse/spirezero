//! `HiddenCache` (`Models.Cards/HiddenCache.cs`): Regent common skill, cost 1; gain 1 star and 3 Star Next Turn (4 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct HiddenCache;

#[sts2_macros::hooks]
impl Hooks for HiddenCache {
    type Me = CardId;
}

impl CardModel for HiddenCache {
    fn name(&self) -> &'static str {
        "Hidden Cache"
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
            CardVar::new(Var::Stars, 1),
            CardVar::new(Var::Power(PowerKind::StarNextTurn), 3).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.gain_stars(eng.s.card(play.card).var(Var::Stars))?;
        let amount = eng
            .s
            .card(play.card)
            .var(Var::Power(PowerKind::StarNextTurn));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::StarNextTurn,
            amount,
            Some(player),
        )
    }
}
