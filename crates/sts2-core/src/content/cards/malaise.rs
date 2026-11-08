//! `Malaise` (`Models.Cards/Malaise.cs`): X cost, Exhaust; -X Strength and X Weak on the target
//! (one more of each upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Malaise;

#[sts2_macros::hooks]
impl Hooks for Malaise {
    type Me = CardId;
}

impl CardModel for Malaise {
    fn name(&self) -> &'static str {
        "Malaise"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn has_energy_cost_x(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Malaise needs a target");
        let mut amount = eng.hooked().resolve_energy_x_value(play.card);
        if eng.s.card(play.card).is_upgraded() {
            amount += 1;
        }
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Strength,
            Decimal::from(-amount),
            Some(player),
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Weak,
            Decimal::from(amount),
            Some(player),
        )
    }
}
