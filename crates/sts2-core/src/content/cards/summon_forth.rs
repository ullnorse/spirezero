//! `SummonForth` (`Models.Cards/SummonForth.cs`): Regent uncommon skill, cost 1 (the Retain hover tip is not a keyword); move
//! every Sovereign Blade that is not in the hand into the hand, then Forge 8 (11 upgraded).

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, Listener};
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardModel, CardRarity, CardType, CardVar, TargetType, Var, sovereign_blade,
};

pub struct SummonForth;

#[sts2_macros::hooks]
impl Hooks for SummonForth {
    type Me = CardId;
}

impl CardModel for SummonForth {
    fn name(&self) -> &'static str {
        "Summon Forth"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Forge, 8).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let blades: Vec<CardId> = sovereign_blade::all_blades(eng.s)
            .into_iter()
            .filter(|&c| eng.s.pile_of(c) != Some(PileType::Hand))
            .collect();
        eng.add_cards_to_pile(&blades, PileType::Hand, Position::Bottom)?;
        let forge = eng.s.card(play.card).var(Var::Forge);
        eng.forge(forge, Some(Listener::Card(play.card)))?;
        Ok(())
    }
}
