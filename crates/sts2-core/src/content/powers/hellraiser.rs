//! `HellraiserPower` (`Models.Powers/HellraiserPower.cs`): a buff (`Single`); each Strike-tagged
//! card the owner draws is played at once with no target. While every hittable enemy's HP display
//! is infinite (the Waterfall Giant about to blow, a full Hardened Shell), at most 9 a turn are;
//! any other draw, and the owner's turn end, reset the count. The cap message is UI.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerModel, PowerType};

pub struct Hellraiser;

/// `_infiniteAutoPlayCap`.
const INFINITE_AUTO_PLAY_CAP: i32 = 9;

/// `HellraiserPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `autoPlayingCards.Count`: the cards being auto-played, nested. A card being played is
    /// never drawn, so the set never holds one twice.
    pub auto_playing_cards: u32,
    /// `infiniteAutoPlaysThisTurn`.
    pub infinite_auto_plays_this_turn: i32,
    /// `showedCapReachedMessage`.
    pub showed_cap_reached_message: bool,
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> &mut Data {
    match s.power_data_mut(me.id) {
        PowerData::Hellraiser(data) => data,
        _ => unreachable!("a Hellraiser instance holds Hellraiser data"),
    }
}

/// `ResetInfiniteAutoPlayData`.
fn reset_infinite_auto_play_data(s: &mut CombatState, me: PowerRef) {
    let data = data_mut(s, me);
    data.infinite_auto_plays_this_turn = 0;
    data.showed_cap_reached_message = false;
}

impl PowerModel for Hellraiser {
    /// Auto-plays each drawn Strike; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Hellraiser"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Hellraiser(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Hellraiser {
    type Me = PowerRef;

    fn after_card_drawn_early(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        // `card.Owner.Creature != Owner`: every card belongs to the player.
        if eng.s.player.creature != me.owner || !eng.s.card(card).kind.is_strike() {
            return Ok(());
        }
        let infinite = eng
            .s
            .hittable_enemies()
            .iter()
            .all(|&e| eng.s.creature(e).infinite_hp_display);
        let mut play = true;
        if infinite {
            let data = data_mut(eng.s, me);
            if data.infinite_auto_plays_this_turn >= INFINITE_AUTO_PLAY_CAP {
                play = false;
                // `ThinkCmd.Play` (the cap message) is UI.
                data.showed_cap_reached_message = true;
            }
            data.infinite_auto_plays_this_turn += 1;
        } else {
            reset_infinite_auto_play_data(eng.s, me);
        }
        if play {
            data_mut(eng.s, me).auto_playing_cards += 1;
            eng.auto_play(card, None)?;
            data_mut(eng.s, me).auto_playing_cards -= 1;
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
            reset_infinite_auto_play_data(eng.s, me);
        }
        Ok(())
    }
}
