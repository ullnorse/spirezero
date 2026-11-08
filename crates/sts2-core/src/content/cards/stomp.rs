//! `Stomp` (`Models.Cards/Stomp.cs`): cost 3; 12 damage (15 upgraded) to all enemies. Costs 1
//! less this turn for every Attack the owner played this turn (counted when it enters combat,
//! then at each Attack play).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, HistoryKind, HistoryTag};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Stomp;

/// `ReduceCostBy`: `EnergyCost.AddThisTurn(-amount)`.
fn reduce_cost_by(eng: &mut Engine<'_>, me: CardId, amount: i32) {
    eng.s.energy_cost_mut(me).add_this_turn(-amount);
}

#[sts2_macros::hooks]
impl Hooks for Stomp {
    type Me = CardId;

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: CardId, card: CardId) -> Flow {
        if card != me || eng.s.card(me).is_clone() {
            return Ok(());
        }
        // `CardPlaysFinished` of the owner's Attacks this turn.
        let played = eng
            .s
            .history_this_turn(HistoryTag::CardPlayFinished)
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    HistoryKind::CardPlayFinished {
                        card_type: CardType::Attack,
                        ..
                    }
                )
            })
            .count();
        reduce_cost_by(eng, me, i32::try_from(played).expect("a count fits i32"));
        Ok(())
    }

    fn before_card_played(&self, eng: &mut Engine<'_>, me: CardId, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner != Owner`: every card belongs to the player.
        if eng.s.card(play.card).card_type() == CardType::Attack {
            reduce_cost_by(eng, me, 1);
        }
        Ok(())
    }
}

impl CardModel for Stomp {
    fn name(&self) -> &'static str {
        "Stomp"
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
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 12).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
