//! `Aggression` (`Models.Cards/Aggression.cs`): cost 1; apply `AggressionPower` (1). Upgraded, it gains Innate.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Aggression;

#[sts2_macros::hooks]
impl Hooks for Aggression {
    type Me = CardId;
}

impl CardModel for Aggression {
    fn name(&self) -> &'static str {
        "Aggression"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Innate);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // CreatureCmd.TriggerAnim is visual only.
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Aggression,
            Decimal::ONE,
            Some(player),
        )
    }
}
