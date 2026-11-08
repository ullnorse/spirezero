//! `Apotheosis` (`Models.Cards/Apotheosis.cs`): Ancient (`JewelryBox`) skill, cost 2, Exhaust,
//! Innate; upgrades every other upgradable card in every pile (`PlayerCombatState.AllCards`).
//! Upgrade: cost 1.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType};

pub struct Apotheosis;

#[sts2_macros::hooks]
impl Hooks for Apotheosis {
    type Me = CardId;
}

impl CardModel for Apotheosis {
    fn name(&self) -> &'static str {
        "Apotheosis"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE
            .with(CardKeyword::Exhaust)
            .with(CardKeyword::Innate)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let all: Vec<CardId> = PileType::LISTENER_ORDER
            .into_iter()
            .flat_map(|p| eng.s.player.pile(p).iter().copied())
            .collect();
        for card in all {
            if card != play.card && eng.s.card(card).is_upgradable() {
                eng.upgrade_card(card);
            }
        }
        Ok(())
    }
}
