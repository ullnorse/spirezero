//! `PillarOfCreation` (`Models.Cards/PillarOfCreation.cs`): Regent uncommon power, cost 1; apply
//! `PillarOfCreationPower` with 2 (3 upgraded; the `BlockVar`'s base value).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct PillarOfCreation;

#[sts2_macros::hooks]
impl Hooks for PillarOfCreation {
    type Me = CardId;
}

impl CardModel for PillarOfCreation {
    fn name(&self) -> &'static str {
        "Pillar of Creation"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 2)
            .props(ValueProps::UNPOWERED)
            .upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::PillarOfCreation,
            eng.s.card(play.card).var(Var::Block),
            Some(player),
        )
    }
}
