//! `BattleTrance` (`Models.Cards/BattleTrance.cs`): cost 0; draw 3 cards (4 upgraded), then you
//! cannot draw more cards this turn (`NoDrawPower`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct BattleTrance;

#[sts2_macros::hooks]
impl Hooks for BattleTrance {
    type Me = CardId;
}

impl CardModel for BattleTrance {
    fn name(&self) -> &'static str {
        "Battle Trance"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).int_var(Var::Cards);
        eng.draw(Decimal::from(cards), false)?;
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::NoDraw,
            Decimal::ONE,
            Some(player),
        )
    }
}
