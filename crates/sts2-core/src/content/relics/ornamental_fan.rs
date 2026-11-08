//! `OrnamentalFan` (`Models.Relics/OrnamentalFan.cs`): every 3rd Attack the owner plays in a turn
//! gives 4 Block. `AfterCombatEnd` only clears `IsActivating` (a visual), so the count outlives
//! the fight until the next turn start resets it.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::cards::CardType;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(3)`.
const CARDS: i32 = 3;
/// `BlockVar(4m, ValueProp.Unpowered)`.
const BLOCK: i32 = 4;

pub struct OrnamentalFan;

/// `OrnamentalFan`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `AttacksPlayedThisTurn`.
    pub attacks_played_this_turn: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::OrnamentalFan(data) = &mut s.relic_mut(me).data else {
        unreachable!("an OrnamentalFan has OrnamentalFan data");
    };
    data
}

impl RelicModel for OrnamentalFan {
    fn name(&self) -> &'static str {
        "Ornamental Fan"
    }

    fn init_data(&self) -> RelicData {
        RelicData::OrnamentalFan(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for OrnamentalFan {
    type Me = RelicId;

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            data_mut(eng.s, me).attacks_played_this_turn = 0;
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        if !eng.s.in_progress || eng.s.card(play.card).card_type() != CardType::Attack {
            return Ok(());
        }
        let data = data_mut(eng.s, me);
        data.attacks_played_this_turn += 1;
        if data.attacks_played_this_turn % CARDS == 0 {
            let owner = eng.s.player.creature;
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
