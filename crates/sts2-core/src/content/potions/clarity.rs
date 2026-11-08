//! `Clarity` (`Models.Potions/Clarity.cs`, "Clarity Extract"): the target player draws 1 card,
//! then gains 3 Clarity (a card more in each of the next hand draws).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PowerVar<ClarityPower>(3m)`.
const CLARITY: i32 = 3;
/// `CardsVar(1)`.
const CARDS: i32 = 1;

pub struct Clarity;

#[sts2_macros::hooks]
impl Hooks for Clarity {
    type Me = PotionId;
}

impl PotionModel for Clarity {
    fn name(&self) -> &'static str {
        "Clarity Extract"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        let owner = eng.s.player.creature;
        // CardPileCmd.Draw(ctx, Cards.BaseValue, target.Player): the only player.
        eng.draw(Decimal::from(CARDS), false)?;
        eng.apply_power(
            target,
            PowerKind::Clarity,
            Decimal::from(CLARITY),
            Some(owner),
        )
    }
}
