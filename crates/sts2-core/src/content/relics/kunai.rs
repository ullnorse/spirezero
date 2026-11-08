//! `Kunai` (`Models.Relics/Kunai.cs`): every 3rd Attack the owner plays in a turn gives 1
//! Dexterity.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::cards::CardType;
use crate::content::powers::PowerKind;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(3)`.
const CARDS: i32 = 3;
/// `PowerVar<DexterityPower>(1m)`.
const DEXTERITY: i32 = 1;

pub struct Kunai;

/// `Kunai`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `AttacksPlayedThisTurn`.
    pub attacks_played_this_turn: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::Kunai(data) = &mut s.relic_mut(me).data else {
        unreachable!("a Kunai has Kunai data");
    };
    data
}

impl RelicModel for Kunai {
    fn name(&self) -> &'static str {
        "Kunai"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Kunai(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Kunai {
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
                PowerKind::Dexterity,
                Decimal::from(DEXTERITY),
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
