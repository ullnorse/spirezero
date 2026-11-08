//! `Melancholy` (`Models.Cards/Melancholy.cs`): cost 3, 13 Block (17 upgraded). Costs 1 less
//! for the rest of the combat whenever any creature dies.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CreatureId, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Melancholy;

#[sts2_macros::hooks]
impl Hooks for Melancholy {
    type Me = CardId;

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: CardId,
        _creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        if was_removal_prevented {
            return Ok(());
        }
        // `Pile.IsCombatPile`: the engine only dispatches to cards in a combat pile.
        let energy = eng.s.card(me).int_var(Var::Energy);
        eng.s.energy_cost_mut(me).add_this_combat(-energy);
        Ok(())
    }
}

impl CardModel for Melancholy {
    fn name(&self) -> &'static str {
        "Melancholy"
    }

    fn cost(&self) -> i32 {
        3
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
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 13).upgrade(4),
            CardVar::new(Var::Energy, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}
