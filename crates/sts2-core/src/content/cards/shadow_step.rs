//! `ShadowStep` (`Models.Cards/ShadowStep.cs`): cost 1 (0 upgraded), discard the whole hand, then
//! apply `ShadowStepPower`.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct ShadowStep;

#[sts2_macros::hooks]
impl Hooks for ShadowStep {
    type Me = CardId;
}

impl CardModel for ShadowStep {
    fn name(&self) -> &'static str {
        "Shadow Step"
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

    fn on_upgrade(&self, card: &mut crate::combat::state::Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let hand: Vec<CardId> = eng.s.player.hand.to_vec();
        eng.discard(&hand)?;
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::ShadowStep,
            Decimal::ONE,
            Some(player),
        )
    }
}
