//! `Spur` (`Models.Cards/Spur.cs`): cost 1, Retain, Summon 3 (5 upgraded), then heal Osty 5 (7).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Spur;

#[sts2_macros::hooks]
impl Hooks for Spur {
    type Me = CardId;
}

impl CardModel for Spur {
    fn name(&self) -> &'static str {
        "Spur"
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
        Keywords::NONE.with(CardKeyword::Retain)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Summon, 3).upgrade(2),
            CardVar::new(Var::Heal, 5).upgrade(2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.summon_osty(eng.s.card(play.card).var(Var::Summon))?;
        // `Owner.Osty` is non-null after a summon that was not vetoed (Summon 0 is a no-op, but
        // the amount here is never 0). Healing a dead Osty cannot happen: the summon revived it.
        let osty = eng.s.osty().expect("Spur summoned Osty");
        eng.heal(osty, eng.s.card(play.card).var(Var::Heal))
    }
}
