//! `SmoggyPower` (`Models.Powers/SmoggyPower.cs`): a debuff (`Single`) that afflicts every Skill of
//! its owner's player with `Smog` after the owner plays a Skill, and every Skill that enters combat
//! after a Skill play this turn. A `Smog` card cannot be played. The afflictions clear when the
//! owner's turn ends; the power stays.

use crate::combat::AutoPlayType;
use crate::combat::history::{HistoryKind, HistoryTag};
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::afflictions::AfflictionKind;
use crate::content::cards::CardType;
use crate::content::powers::tangled::all_cards;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Smoggy;

fn is_skill(s: &CombatState, card: CardId) -> bool {
    s.card(card).card_type() == CardType::Skill
}

fn is_smog(s: &CombatState, card: CardId) -> bool {
    s.card(card)
        .affliction
        .is_some_and(|a| a.kind == AfflictionKind::Smog)
}

impl PowerModel for Smoggy {
    fn name(&self) -> &'static str {
        "Smoggy"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Smoggy {
    type Me = PowerRef;

    /// The owner is the only player, so every played card is its player's.
    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        if !eng.s.creature(me.owner).is_player() || !is_skill(eng.s, play.card) {
            return Ok(());
        }
        for card in all_cards(eng.s) {
            if is_skill(eng.s, card) && eng.s.card(card).affliction.is_none() {
                eng.afflict(card, AfflictionKind::Smog, Decimal::ONE);
            }
        }
        Ok(())
    }

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: PowerRef, card: CardId) -> Flow {
        let s = &*eng.s;
        // `CardPlaysStarted.Any(e => HappenedThisTurn && Card.Type == Skill && Player == owner's)`.
        let skill_started =
            s.history_this_turn(HistoryTag::CardPlayStarted)
                .iter()
                .any(|e| match &e.kind {
                    HistoryKind::CardPlayStarted(play) => is_skill(s, play.card),
                    _ => false,
                });
        if s.creature(me.owner).is_player()
            && s.card(card).affliction.is_none()
            && is_skill(s, card)
            && skill_started
        {
            eng.afflict(card, AfflictionKind::Smog, Decimal::ONE);
        }
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            for card in all_cards(eng.s) {
                if is_smog(eng.s, card) {
                    eng.clear_affliction(card);
                }
            }
        }
        Ok(())
    }

    /// `ShouldPlay(card, _)`: the type of play is ignored.
    fn should_play(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _auto_play: AutoPlayType,
    ) -> bool {
        me.owner != s.player.creature || !is_smog(s, card)
    }
}
