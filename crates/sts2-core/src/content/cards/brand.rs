//! `Brand` (`Models.Cards/Brand.cs`): lose 1 HP (unblockable, unpowered), exhaust a card of the
//! hand (the player's pick), then gain 1 Strength (2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Brand;

#[sts2_macros::hooks]
impl Hooks for Brand {
    type Me = CardId;
}

impl CardModel for Brand {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Brand"
    }

    fn cost(&self) -> i32 {
        0
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::HpLoss, 1),
            CardVar::new(Var::Power(PowerKind::Strength), 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.damage(
            &[player],
            eng.s.card(play.card).var(Var::HpLoss),
            ValueProps::UNBLOCKABLE
                .with(ValueProps::UNPOWERED)
                .with(ValueProps::MOVE),
            Some(player),
            Some(play.card),
        )?;
        let options = eng.s.player.hand.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Exhaust), options, 1, 1)?
            .first()
        {
            eng.exhaust(card, false)?;
        }
        let strength = eng.s.card(play.card).var(Var::Power(PowerKind::Strength));
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Strength,
            strength,
            Some(player),
        )
    }
}
