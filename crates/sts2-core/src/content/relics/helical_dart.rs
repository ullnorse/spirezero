//! `HelicalDart` (`Models.Relics/HelicalDart.cs`): after the owner plays a Shiv, gains 1
//! Dexterity until the end of the turn (`HelicalDartPower`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<DexterityPower>(1m)`.
const DEXTERITY: i32 = 1;

pub struct HelicalDart;

impl RelicModel for HelicalDart {
    fn name(&self) -> &'static str {
        "Helical Dart"
    }
}

#[sts2_macros::hooks]
impl Hooks for HelicalDart {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, _me: RelicId, play: &CardPlay) -> Flow {
        if !eng.s.card(play.card).kind.is_shiv() {
            return Ok(());
        }
        // Flash: visual only.
        let owner = eng.s.player.creature;
        eng.apply_power(
            owner,
            PowerKind::HelicalDart,
            Decimal::from(DEXTERITY),
            Some(owner),
        )
    }
}
