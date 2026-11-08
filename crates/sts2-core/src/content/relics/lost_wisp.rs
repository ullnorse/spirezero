//! `LostWisp` (`Models.Relics/LostWisp.cs`): whenever the owner plays a Power, 8 damage to every
//! hittable enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, Engine, Flow, RelicId, ValueProps};
use crate::content::cards::CardType;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DamageVar(8m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 8;

pub struct LostWisp;

impl RelicModel for LostWisp {
    fn name(&self) -> &'static str {
        "Lost Wisp"
    }
}

#[sts2_macros::hooks]
impl Hooks for LostWisp {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, _me: RelicId, play: &CardPlay) -> Flow {
        if !eng.s.in_progress || eng.s.card(play.card).card_type() != CardType::Power {
            return Ok(());
        }
        let owner = eng.s.player.creature;
        let targets = eng.hooked().hittable_enemies();
        eng.damage(
            &targets,
            Decimal::from(DAMAGE),
            ValueProps::UNPOWERED,
            Some(owner),
            None,
        )?;
        Ok(())
    }
}
