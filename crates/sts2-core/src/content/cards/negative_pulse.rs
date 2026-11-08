//! `NegativePulse` (`Models.Cards/NegativePulse.cs`): cost 1; 5 Block (6 upgraded) and 7 Doom (11)
//! to all enemies.

use crate::combat::ValueProps;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct NegativePulse;

#[sts2_macros::hooks]
impl Hooks for NegativePulse {
    type Me = CardId;
}

impl CardModel for NegativePulse {
    fn name(&self) -> &'static str {
        "Negative Pulse"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 5).upgrade(1),
            CardVar::new(Var::Power(PowerKind::Doom), 7).upgrade(4),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let doom = eng.s.card(play.card).var(Var::Power(PowerKind::Doom));
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        for enemy in eng.hooked().hittable_enemies() {
            eng.apply_power_from_card(Some(play.card), enemy, PowerKind::Doom, doom, Some(player))?;
        }
        Ok(())
    }
}
