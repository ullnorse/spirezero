//! `Lethality` (`Models.Cards/Lethality.cs`): cost 1, Ethereal; the first Attack each turn deals 50% (75% upgraded) more damage.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Lethality;

#[sts2_macros::hooks]
impl Hooks for Lethality {
    type Me = CardId;
}

impl CardModel for Lethality {
    fn name(&self) -> &'static str {
        "Lethality"
    }

    fn cost(&self) -> i32 {
        1
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Ethereal)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Lethality), 50).upgrade(25)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Lethality));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Lethality, amount, Some(player))
    }
}
