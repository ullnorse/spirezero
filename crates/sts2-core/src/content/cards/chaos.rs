//! `Chaos` (`Models.Cards/Chaos.cs`): cost 1; channel 1 random orb (2 upgraded), each drawn by
//! `GetRandomOrb` on `Rng.CombatOrbGeneration` (the `CombatOrbs` stream).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Chaos;

#[sts2_macros::hooks]
impl Hooks for Chaos {
    type Me = CardId;
}

impl CardModel for Chaos {
    fn name(&self) -> &'static str {
        "Chaos"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Repeat, 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let repeat = eng.s.card(play.card).int_var(Var::Repeat);
        for _ in 0..repeat {
            let kind = eng.random_orb();
            eng.channel(kind)?;
        }
        Ok(())
    }
}
