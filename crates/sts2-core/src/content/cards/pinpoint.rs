//! `Pinpoint` (`Models.Cards/Pinpoint.cs`): cost 3, 15 damage (19 upgraded). Costs 1 less this turn per Skill played this turn.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, HistoryKind, HistoryTag};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Pinpoint;

#[sts2_macros::hooks]
impl Hooks for Pinpoint {
    type Me = CardId;

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: CardId, card: CardId) -> Flow {
        if card != me || eng.s.card(me).is_clone() {
            return Ok(());
        }
        let player = eng.s.player.creature;
        let amount = eng
            .s
            .history_this_turn(HistoryTag::CardPlayFinished)
            .iter()
            .filter(|e| {
                e.actor == player
                    && matches!(
                        e.kind,
                        HistoryKind::CardPlayFinished {
                            card_type: CardType::Skill,
                            ..
                        }
                    )
            })
            .count();
        reduce_cost_by(eng, me, amount);
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: CardId, play: &CardPlay) -> Flow {
        if eng.s.card(play.card).card_type() == CardType::Skill {
            reduce_cost_by(eng, me, 1);
        }
        Ok(())
    }
}

impl CardModel for Pinpoint {
    fn name(&self) -> &'static str {
        "Pinpoint"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 15).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Pinpoint needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}

/// `ReduceCostBy`: `EnergyCost.AddThisTurn(-amount)`.
fn reduce_cost_by(eng: &mut Engine<'_>, me: CardId, amount: usize) {
    let amount = i32::try_from(amount).expect("a turn has few card plays");
    eng.s.energy_cost_mut(me).add_this_turn(-amount);
}
