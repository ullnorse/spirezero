//! `BulkUp` (`Models.Cards/BulkUp.cs`): cost 2 power; lose 1 orb slot, then 2 Strength and 2
//! Dexterity (3 each upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct BulkUp;

#[sts2_macros::hooks]
impl Hooks for BulkUp {
    type Me = CardId;
}

impl CardModel for BulkUp {
    fn name(&self) -> &'static str {
        "Bulk Up"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Named("OrbSlots"), 1),
            CardVar::new(Var::Power(PowerKind::Strength), 2).upgrade(1),
            CardVar::new(Var::Power(PowerKind::Dexterity), 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let card = eng.s.card(play.card);
        let slots = card.int_var(Var::Named("OrbSlots"));
        let strength = card.var(Var::Power(PowerKind::Strength));
        let dexterity = card.var(Var::Power(PowerKind::Dexterity));
        let player = eng.s.player.creature;
        eng.remove_orb_slots(slots);
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Strength,
            strength,
            Some(player),
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Dexterity,
            dexterity,
            Some(player),
        )
    }
}
