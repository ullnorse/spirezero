//! `Shuriken` (`Models.Relics/Shuriken.cs`): every 3rd Attack the owner plays in a turn gives 1
//! Strength.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::cards::CardType;
use crate::content::powers::PowerKind;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(3)`.
const CARDS: i32 = 3;
/// `PowerVar<StrengthPower>(1m)`.
const STRENGTH: i32 = 1;

pub struct Shuriken;

/// `Shuriken`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `AttacksPlayedThisTurn`.
    pub attacks_played_this_turn: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::Shuriken(data) = &mut s.relic_mut(me).data else {
        unreachable!("a Shuriken has Shuriken data");
    };
    data
}

impl RelicModel for Shuriken {
    fn name(&self) -> &'static str {
        "Shuriken"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Shuriken(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Shuriken {
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
            eng.apply_power(
                owner,
                PowerKind::Strength,
                Decimal::from(STRENGTH),
                Some(owner),
            )?;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).attacks_played_this_turn = 0;
        Ok(())
    }
}
