//! `Expose` (`Models.Cards/Expose.cs`): cost 0, Exhaust; strips the target's Block, removes its
//! Artifact, then applies 2 Vulnerable (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Expose;

#[sts2_macros::hooks]
impl Hooks for Expose {
    type Me = CardId;
}

impl CardModel for Expose {
    fn name(&self) -> &'static str {
        "Expose"
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
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Power"), 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Expose needs a target");
        let amount = eng.s.card(play.card).var(Var::Named("Power"));
        let player = eng.s.player.creature;
        let block = Decimal::from(eng.s.creature(target).block);
        eng.lose_block(target, block, Some(player))?;
        let artifact = eng
            .s
            .creature(target)
            .powers
            .iter()
            .find(|p| p.kind == PowerKind::Artifact)
            .map(|p| p.id);
        if let Some(artifact) = artifact {
            eng.remove_power(target, artifact)?;
        }
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Vulnerable,
            amount,
            Some(player),
        )
    }
}
