//! `Monologue` (`Models.Cards/Monologue.cs`): Regent uncommon skill, cost 0 (Retain upgraded); gain 1 Monologue: Strength per card played this turn.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::content::powers::{PowerKind, monologue};
use crate::decimal::Decimal;

pub struct Monologue;

#[sts2_macros::hooks]
impl Hooks for Monologue {
    type Me = CardId;
}

impl CardModel for Monologue {
    fn name(&self) -> &'static str {
        "Monologue"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Retain);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Power"), 1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let strength = eng.s.card(play.card).int_var(Var::Named("Power"));
        let power = eng.apply_power_instance_from_card(
            Some(play.card),
            player,
            PowerKind::Monologue,
            Decimal::ONE,
            Some(player),
        )?;
        if let Some(power) = power {
            monologue::set_strength(eng.s, power, strength);
        }
        Ok(())
    }
}
