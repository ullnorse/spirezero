//! `LegSweep` (`Models.Cards/LegSweep.cs`): cost 2, 11 Block (14 upgraded), then 2 Weak (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct LegSweep;

#[sts2_macros::hooks]
impl Hooks for LegSweep {
    type Me = CardId;
}

impl CardModel for LegSweep {
    fn name(&self) -> &'static str {
        "Leg Sweep"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 11).upgrade(3),
            CardVar::new(Var::Power(PowerKind::Weak), 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("LegSweep needs a target");
        let block = eng.s.card(play.card).var(Var::Block);
        let weak = eng.s.card(play.card).var(Var::Power(PowerKind::Weak));
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.apply_power_from_card(Some(play.card), target, PowerKind::Weak, weak, Some(player))
    }
}
