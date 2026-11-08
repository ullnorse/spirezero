//! `Sacrifice` (`Models.Cards/Sacrifice.cs`): cost 1 (0 upgraded), Retain; Osty dies and the
//! player gains `3 × Osty.MaxHp` Block. Does nothing without a living Osty.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, ValueProps};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct Sacrifice;

#[sts2_macros::hooks]
impl Hooks for Sacrifice {
    type Me = CardId;
}

impl CardModel for Sacrifice {
    fn name(&self) -> &'static str {
        "Sacrifice"
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
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Retain)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::CalculatedBlock, osty_max_hp_times_3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // Osty.CheckMissingWithAnim.
        if !eng.s.is_osty_alive() {
            return Ok(());
        }
        let osty = eng.s.osty().expect("Osty is alive");
        // Calculated before Osty dies.
        let block = eng
            .s
            .calculate(play.card, Var::CalculatedBlock, play.target)
            .expect("Sacrifice has a CalculatedBlock var");
        eng.kill(&[osty])?;
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}

/// `card.Owner.IsOstyAlive ? card.Owner.Osty.MaxHp * 3 : 0`.
fn osty_max_hp_times_3(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let max_hp = s
        .osty()
        .filter(|_| s.is_osty_alive())
        .map_or(0, |o| s.creature(o).max_hp);
    Decimal::from(max_hp * 3)
}
