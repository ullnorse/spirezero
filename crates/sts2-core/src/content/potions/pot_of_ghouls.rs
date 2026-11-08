//! `PotOfGhouls` (`Models.Potions/PotOfGhouls.cs`): add 2 Souls to the target player's hand.

use crate::combat::PileType;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::CardKind;
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};

/// `CardsVar(2)`.
const CARDS: u32 = 2;

pub struct PotOfGhouls;

#[sts2_macros::hooks]
impl Hooks for PotOfGhouls {
    type Me = PotionId;
}

impl PotionModel for PotOfGhouls {
    fn name(&self) -> &'static str {
        "Pot of Ghouls"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // Soul.CreateInHand(target.Player, Cards.IntValue, combat, Owner): create all, then add
        // each with AddGeneratedCardsToCombat(Hand, creator).
        let creator = eng.s.player.creature;
        let souls: Vec<_> = (0..CARDS)
            .map(|_| eng.create_card(CardKind::Soul))
            .collect();
        for soul in souls {
            eng.add_generated_card_to_combat(soul, PileType::Hand, Some(creator))?;
        }
        Ok(())
    }
}
