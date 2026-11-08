//! `Offering` (`Models.Cards/Offering.cs`): cost 0, Exhaust, loses 6 HP (unblockable, unpowered), gains 2 energy and draws 3 cards (5 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct Offering;

#[sts2_macros::hooks]
impl Hooks for Offering {
    type Me = CardId;
}

impl CardModel for Offering {
    fn name(&self) -> &'static str {
        "Offering"
    }

    fn cost(&self) -> i32 {
        0
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::HpLoss, 6),
            CardVar::new(Var::Energy, 2),
            CardVar::new(Var::Cards, 3).upgrade(2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.damage(
            &[player],
            eng.s.card(play.card).var(Var::HpLoss),
            ValueProps::UNBLOCKABLE
                .with(ValueProps::UNPOWERED)
                .with(ValueProps::MOVE),
            Some(player),
            Some(play.card),
        )?;
        eng.gain_energy(Decimal::from(eng.s.card(play.card).int_var(Var::Energy)))?;
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        Ok(())
    }
}
