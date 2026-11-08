//! `Friendship` (`Models.Cards/Friendship.cs`): cost 1 power; lose 2 Strength (1 upgraded), gain
//! 1 max energy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Friendship;

#[sts2_macros::hooks]
impl Hooks for Friendship {
    type Me = CardId;
}

impl CardModel for Friendship {
    fn name(&self) -> &'static str {
        "Friendship"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Strength), 2).upgrade(-1),
            CardVar::new(Var::Energy, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let strength = Decimal::ZERO - eng.s.card(play.card).var(Var::Power(PowerKind::Strength));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Strength, strength, Some(player))?;
        let energy = eng.s.card(play.card).var(Var::Energy);
        eng.apply_power(player, PowerKind::Friendship, energy, Some(player))
    }
}
