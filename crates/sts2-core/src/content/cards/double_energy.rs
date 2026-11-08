//! `DoubleEnergy` (`Models.Cards/DoubleEnergy.cs`): cost 1 (0 upgraded), Exhaust; gain as much energy as is left.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};
use crate::decimal::Decimal;

pub struct DoubleEnergy;

#[sts2_macros::hooks]
impl Hooks for DoubleEnergy {
    type Me = CardId;
}

impl CardModel for DoubleEnergy {
    fn name(&self) -> &'static str {
        "Double Energy"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
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
        // `PlayerCmd.GainEnergy(Owner.PlayerCombatState.Energy, Owner)`.
        let energy = Decimal::from(eng.s.player.energy);
        eng.gain_energy(energy)
    }
}
