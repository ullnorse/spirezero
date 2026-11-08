//! `RippleBasin` (`Models.Relics/RippleBasin.cs`): 4 Block at the end of a turn in which the owner
//! played no Attack. (`Status`, set by the other hooks, only drives the relic's glow.)

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, HistoryKind, HistoryTag, RelicId, Side, ValueProps};
use crate::content::cards::CardType;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(4m, ValueProp.Unpowered)`.
const BLOCK: i32 = 4;

pub struct RippleBasin;

impl RelicModel for RippleBasin {
    fn name(&self) -> &'static str {
        "Ripple Basin"
    }
}

#[sts2_macros::hooks]
impl Hooks for RippleBasin {
    type Me = RelicId;

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        // `e.CardPlay.Player == Owner`: the only player.
        let played_attack = eng
            .s
            .history_this_turn(HistoryTag::CardPlayFinished)
            .iter()
            .any(|e| {
                matches!(
                    e.kind,
                    HistoryKind::CardPlayFinished {
                        card_type: CardType::Attack,
                        ..
                    }
                )
            });
        if participants.contains(&owner) && !played_attack {
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
