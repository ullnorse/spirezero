//! `FeedingFrenzy` (`Models.Cards/FeedingFrenzy.cs`): Event skill, cost 0; gain 5 Strength
//! (7 upgraded) until the end of the turn (`FeedingFrenzyPower`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct FeedingFrenzy;

#[sts2_macros::hooks]
impl Hooks for FeedingFrenzy {
    type Me = CardId;
}

impl CardModel for FeedingFrenzy {
    fn name(&self) -> &'static str {
        "Feeding Frenzy"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Strength), 5).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let strength = eng.s.card(play.card).var(Var::Power(PowerKind::Strength));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::FeedingFrenzy,
            strength,
            Some(player),
        )
    }
}
