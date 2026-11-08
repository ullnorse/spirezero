//! `BiiigHug` (`Models.Relics/BiiigHug.cs`): Ancient (Tezcatara); `AfterObtained` removes 4
//! chosen deck cards (`FromDeckForRemoval`); after every shuffle of the owner's draw pile, a
//! `Soot` joins the draw pile at a random position.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, PileType, RelicId};
use crate::content::cards::CardKind;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(4)`.
const CARDS: usize = 4;

pub struct BiiigHug;

impl RelicModel for BiiigHug {
    fn name(&self) -> &'static str {
        "Biiig Hug"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().removable_cards();
        ctx.select(SelectPurpose::Remove { damage: 0 }, options, CARDS)
    }
}

#[sts2_macros::hooks]
impl Hooks for BiiigHug {
    type Me = RelicId;

    fn after_shuffle(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // `shuffler == Owner`: the only player.
        let owner = eng.s.player.creature;
        let soot = eng.create_card(CardKind::Soot);
        eng.add_generated_card_to_combat_at(soot, PileType::Draw, Some(owner), Position::Random)
    }
}
