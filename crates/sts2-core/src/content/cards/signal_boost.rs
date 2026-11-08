//! `SignalBoost` (`Models.Cards/SignalBoost.cs`): Defect rare skill, cost 1 (0 upgraded), Exhaust;
//! gain 1 `SignalBoostPower`, so the next Power card is played twice.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct SignalBoost;

#[sts2_macros::hooks]
impl Hooks for SignalBoost {
    type Me = CardId;
}

impl CardModel for SignalBoost {
    fn name(&self) -> &'static str {
        "Signal Boost"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::SignalBoost), 1)];
        VARS
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::SignalBoost,
            eng.s
                .card(play.card)
                .var(Var::Power(PowerKind::SignalBoost)),
            Some(player),
        )
    }
}
