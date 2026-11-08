//! `Quadcast` (`Models.Cards/Quadcast.cs`): cost 1 (0 upgraded); evoke the front orb 4 times, removing it on the last.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Quadcast;

#[sts2_macros::hooks]
impl Hooks for Quadcast {
    type Me = CardId;
}

impl CardModel for Quadcast {
    fn name(&self) -> &'static str {
        "Quadcast"
    }

    fn cost(&self) -> i32 {
        1
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Repeat, 4)];
        VARS
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        if eng.s.player.orbs.is_empty() {
            return Ok(());
        }
        // Evoke the front orb `Repeat` times, dequeuing on the last.
        let repeat = eng.s.card(play.card).int_var(Var::Repeat);
        for i in 0..repeat {
            eng.evoke_next(i == repeat - 1)?;
        }
        Ok(())
    }
}
