//! `MusicBox` (`Models.Relics/MusicBox.cs`): Ancient (Vakuu); the first Attack played each turn
//! leaves an Ethereal clone of itself in the hand once it has been played.

use crate::combat::hooks::Hooks;
use crate::combat::{
    CardId, CardPlay, CombatState, CreatureId, Engine, Flow, PileType, RelicId, Side,
};
use crate::content::cards::{CardKeyword, CardType};
use crate::content::relics::{RelicData, RelicModel};

pub struct MusicBox;

/// `MusicBox`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_wasUsedThisTurn`.
    pub was_used_this_turn: bool,
    /// `_cardBeingPlayed`.
    pub card_being_played: Option<CardId>,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::MusicBox(data) = &mut s.relic_mut(me).data else {
        unreachable!("a MusicBox has MusicBox data");
    };
    data
}

impl RelicModel for MusicBox {
    fn name(&self) -> &'static str {
        "Music Box"
    }

    fn init_data(&self) -> RelicData {
        RelicData::MusicBox(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for MusicBox {
    type Me = RelicId;

    fn before_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        let card_type = eng.s.card(play.card).card_type();
        let data = data_mut(eng.s, me);
        // `cardPlay.Card.Owner != Owner` is never true: the only player owns every card.
        if data.card_being_played.is_none()
            && !data.was_used_this_turn
            && card_type == CardType::Attack
        {
            data.card_being_played = Some(play.card);
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        if data_mut(eng.s, me).card_being_played != Some(play.card) {
            return Ok(());
        }
        let clone = eng.create_clone(play.card);
        eng.apply_keyword(clone, CardKeyword::Ethereal);
        let owner = eng.s.player.creature;
        eng.add_generated_card_to_combat(clone, PileType::Hand, Some(owner))?;
        let data = data_mut(eng.s, me);
        data.was_used_this_turn = true;
        data.card_being_played = None;
        Ok(())
    }

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            let data = data_mut(eng.s, me);
            data.was_used_this_turn = false;
            data.card_being_played = None;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        let data = data_mut(eng.s, me);
        data.was_used_this_turn = false;
        data.card_being_played = None;
        Ok(())
    }
}
