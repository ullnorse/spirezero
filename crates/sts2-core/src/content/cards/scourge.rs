//! `Scourge` (`Models.Cards/Scourge.cs`): cost 1; 13 Doom (16 upgraded), then draw 1 (2).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Scourge;

#[sts2_macros::hooks]
impl Hooks for Scourge {
    type Me = CardId;
}

impl CardModel for Scourge {
    fn name(&self) -> &'static str {
        "Scourge"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Doom), 13).upgrade(3),
            CardVar::new(Var::Cards, 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Scourge needs a target");
        let doom = eng.s.card(play.card).var(Var::Power(PowerKind::Doom));
        let cards = eng.s.card(play.card).var(Var::Cards);
        let player = eng.s.player.creature;
        eng.apply_power_from_card(Some(play.card), target, PowerKind::Doom, doom, Some(player))?;
        eng.draw(cards, false)?;
        Ok(())
    }
}
