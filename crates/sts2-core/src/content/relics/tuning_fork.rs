//! `TuningFork` (`Models.Relics/TuningFork.cs`): every 10th Skill the owner plays gives 7 Block.
//! The count (`[SavedProperty] SkillsPlayed`) carries over between combats.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, CombatState, Engine, Flow, RelicId, ValueProps};
use crate::content::cards::CardType;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(10)`.
const CARDS: i32 = 10;
/// `BlockVar(7m, ValueProp.Unpowered)`.
const BLOCK: i32 = 7;

pub struct TuningFork;

/// `TuningFork`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `SkillsPlayed`.
    pub skills_played: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::TuningFork(data) = &mut s.relic_mut(me).data else {
        unreachable!("a TuningFork has TuningFork data");
    };
    data
}

impl RelicModel for TuningFork {
    fn name(&self) -> &'static str {
        "Tuning Fork"
    }

    fn init_data(&self) -> RelicData {
        RelicData::TuningFork(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for TuningFork {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        if eng.s.card(play.card).card_type() != CardType::Skill {
            return Ok(());
        }
        let data = data_mut(eng.s, me);
        data.skills_played += 1;
        if data.skills_played >= CARDS {
            let owner = eng.s.player.creature;
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
            data_mut(eng.s, me).skills_played -= CARDS;
        }
        Ok(())
    }
}
