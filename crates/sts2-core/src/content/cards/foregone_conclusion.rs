//! `ForegoneConclusion` (`Models.Cards/ForegoneConclusion.cs`): Regent rare skill, cost 1; apply
//! `Cards` (2, 3 upgraded) `ForegoneConclusionPower`.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct ForegoneConclusion;

#[sts2_macros::hooks]
impl Hooks for ForegoneConclusion {
    type Me = CardId;
}

impl CardModel for ForegoneConclusion {
    fn name(&self) -> &'static str {
        "Foregone Conclusion"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::ForegoneConclusion,
            eng.s.card(play.card).var(Var::Cards),
            Some(player),
        )
    }
}
