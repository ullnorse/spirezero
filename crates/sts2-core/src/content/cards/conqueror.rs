//! `Conqueror` (`Models.Cards/Conqueror.cs`): Regent uncommon skill, cost 1, any enemy; Forge 3
//! (5 upgraded), then apply 1 `ConquerorPower` to the target.

use crate::combat::hooks::{Hooks, Listener};
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Conqueror;

#[sts2_macros::hooks]
impl Hooks for Conqueror {
    type Me = CardId;
}

impl CardModel for Conqueror {
    fn name(&self) -> &'static str {
        "Conqueror"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Forge, 3).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Conqueror needs a target");
        let forge = eng.s.card(play.card).var(Var::Forge);
        let player = eng.s.player.creature;
        eng.forge(forge, Some(Listener::Card(play.card)))?;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Conqueror,
            Decimal::ONE,
            Some(player),
        )
    }
}
