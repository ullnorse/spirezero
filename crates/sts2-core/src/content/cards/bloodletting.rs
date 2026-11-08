//! `Bloodletting` (`Models.Cards/Bloodletting.cs`): cost 0, loses 3 HP (unblockable, unpowered), then gains 2 energy (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Bloodletting;

#[sts2_macros::hooks]
impl Hooks for Bloodletting {
    type Me = CardId;
}

impl CardModel for Bloodletting {
    fn name(&self) -> &'static str {
        "Bloodletting"
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::HpLoss, 3),
            CardVar::new(Var::Energy, 2).upgrade(1),
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
        eng.gain_energy(eng.s.card(play.card).var(Var::Energy))
    }
}
