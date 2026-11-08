//! `BlessedAntler` (`Models.Relics/BlessedAntler.cs`): Ancient (Nonupeipe); +1 max energy, and
//! on the first turn 3 `Dazed` are shuffled into the draw pile before the hand is drawn
//! (`BeforeHandDraw`: all three are created, then added at random positions).

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, Engine, Flow, PileType, RelicId};
use crate::content::cards::CardKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;
/// `CardsVar(3)`.
const CARDS: usize = 3;

pub struct BlessedAntler;

impl RelicModel for BlessedAntler {
    fn name(&self) -> &'static str {
        "Blessed Antler"
    }
}

#[sts2_macros::hooks]
impl Hooks for BlessedAntler {
    type Me = RelicId;

    fn modify_max_energy(&self, _s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        amount + ENERGY
    }

    fn before_hand_draw(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.player.turn_number != 1 {
            return Ok(());
        }
        let cards: Vec<_> = (0..CARDS)
            .map(|_| eng.create_card(CardKind::Dazed))
            .collect();
        let owner = eng.s.player.creature;
        eng.add_generated_cards_to_combat(&cards, PileType::Draw, Some(owner), Position::Random)
    }
}
