//! `Hexed` (`Models.Afflictions/Hexed.cs`): `HexPower` holds the logic (the card is Ethereal
//! while it exists). A Hexed card entering combat while its owner has no `Hex` loses it.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, Engine, Flow};
use crate::content::afflictions::AfflictionModel;
use crate::content::powers::PowerKind;

/// `HexPower`'s affliction: the card is Ethereal.
pub struct Hexed;

impl AfflictionModel for Hexed {
    fn name(&self) -> &'static str {
        "Hexed"
    }
}

#[sts2_macros::hooks]
impl Hooks for Hexed {
    type Me = CardId;

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: CardId, card: CardId) -> Flow {
        if card != me {
            return Ok(());
        }
        // `card.Owner.Creature`: every card belongs to the player.
        if eng
            .s
            .creature(eng.s.player.creature)
            .has_power(PowerKind::Hex)
        {
            return Ok(());
        }
        eng.clear_affliction(me);
        Ok(())
    }
}
