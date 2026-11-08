//! `AggressionPower` (`Models.Powers/AggressionPower.cs`): a buff (`Counter`); before the owner's
//! side starts its turn, `Amount` random Attacks of the discard pile (`CombatCardSelection`
//! shuffle, then take) move to the hand and are upgraded.

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, PileType, Side};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};
use crate::rng::RunRngType;
use crate::shuffle::unstable_shuffle;

pub struct Aggression;

impl PowerModel for Aggression {
    fn name(&self) -> &'static str {
        "Aggression"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Aggression {
    type Me = PowerRef;

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&me.owner) {
            return Ok(());
        }
        let mut attacks: Vec<_> = eng
            .s
            .player
            .discard
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).card_type() == CardType::Attack)
            .collect();
        unstable_shuffle(
            &mut attacks,
            &mut eng.s.rng[RunRngType::CombatCardSelection],
        );
        let amount = usize::try_from(eng.s.power(me.owner, me.id).amount).unwrap_or(0);
        for card in attacks.into_iter().take(amount) {
            eng.add_to_pile(card, PileType::Hand, Position::Bottom)?;
            // `if (card.IsUpgradable) CardCmd.Upgrade(card)`: `upgrade_card` checks it.
            eng.upgrade_card(card);
        }
        Ok(())
    }
}
