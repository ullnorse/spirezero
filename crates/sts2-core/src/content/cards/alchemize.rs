//! `Alchemize` (`Models.Cards/Alchemize.cs`): cost 1 (0 upgraded), Exhaust; procures a random
//! combat potion (`PotionFactory.CreateRandomPotionInCombat` on `CombatPotionGeneration`,
//! `PotionCmd.TryToProcure`: nothing if the belt is full, after the draw). Cannot be generated in
//! combat.
//!
//! Generation stops the combat (`Unsupported`) if it picks a potion that is not ported.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, Unsupported};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};
use crate::rng::RunRngType;
use crate::run::potions::{combat_potion, random_potion_in_combat};

pub struct Alchemize;

#[sts2_macros::hooks]
impl Hooks for Alchemize {
    type Me = CardId;
}

impl CardModel for Alchemize {
    fn name(&self) -> &'static str {
        "Alchemize"
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        let id = random_potion_in_combat(
            eng.s.player.character,
            &mut eng.s.rng[RunRngType::CombatPotionGeneration],
        );
        let Some(kind) = combat_potion(id) else {
            return Unsupported::potion(id).stop();
        };
        eng.try_to_procure_potion(kind)?;
        Ok(())
    }
}
