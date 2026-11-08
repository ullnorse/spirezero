//! `NecroMastery` (`Models.Cards/NecroMastery.cs`): cost 2 power; summons Osty for 5 (8
//! upgraded), then applies 1 Necro Mastery.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct NecroMastery;

#[sts2_macros::hooks]
impl Hooks for NecroMastery {
    type Me = CardId;
}

impl CardModel for NecroMastery {
    fn name(&self) -> &'static str {
        "Necro Mastery"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Summon, 5).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let summon = eng.s.card(play.card).int_var(Var::Summon);
        eng.summon_osty(Decimal::from(summon))?;
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::NecroMastery, Decimal::ONE, Some(player))
    }
}
