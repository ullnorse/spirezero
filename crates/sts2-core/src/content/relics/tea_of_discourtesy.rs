//! `TeaOfDiscourtesy` (`Models.Relics/TeaOfDiscourtesy.cs`): the next combat starts with 2
//! `Dazed` shuffled into the draw pile (`CombatsLeft`, 1, a `[SavedProperty]`).

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, PileType, RelicId};
use crate::content::cards::CardKind;
use crate::content::relics::{RelicData, RelicModel};

/// `DynamicVar("DazedCount", 2m)`.
const DAZED_COUNT: u32 = 2;

pub struct TeaOfDiscourtesy;

/// `TeaOfDiscourtesy`'s fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CombatsLeft`.
    pub combats_left: i32,
}

impl RelicModel for TeaOfDiscourtesy {
    fn name(&self) -> &'static str {
        "Tea of Discourtesy"
    }

    /// `_combatsLeft = 1`.
    fn init_data(&self) -> RelicData {
        RelicData::TeaOfDiscourtesy(Data { combats_left: 1 })
    }
}

#[sts2_macros::hooks]
impl Hooks for TeaOfDiscourtesy {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        let RelicData::TeaOfDiscourtesy(data) = eng.s.relic(me).data else {
            unreachable!("a TeaOfDiscourtesy has TeaOfDiscourtesy data");
        };
        if data.combats_left <= 0 {
            return Ok(());
        }
        // `AddToCombatAndPreview<Dazed>(Owner.Creature, Draw, 2, Owner, Random)`: each card is
        // created and added in turn.
        let owner = eng.s.player.creature;
        if eng.s.creature(owner).is_alive() {
            for _ in 0..DAZED_COUNT {
                let card = eng.create_card(CardKind::Dazed);
                eng.add_generated_card_to_combat_at(
                    card,
                    PileType::Draw,
                    Some(owner),
                    Position::Random,
                )?;
            }
        }
        let RelicData::TeaOfDiscourtesy(data) = &mut eng.s.relic_mut(me).data else {
            unreachable!("a TeaOfDiscourtesy has TeaOfDiscourtesy data");
        };
        data.combats_left -= 1;
        Ok(())
    }
}
