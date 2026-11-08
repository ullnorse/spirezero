//! `BigBang` (`Models.Cards/BigBang.cs`): Regent rare skill, cost 0, Exhaust; draw 1, gain 1
//! star, gain 1 energy, Forge 5. Upgrade: Innate.

use crate::combat::hooks::{Hooks, Listener};
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct BigBang;

#[sts2_macros::hooks]
impl Hooks for BigBang {
    type Me = CardId;
}

impl CardModel for BigBang {
    fn name(&self) -> &'static str {
        "Big Bang"
    }

    fn cost(&self) -> i32 {
        0
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
        card.keywords.insert(CardKeyword::Innate);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Cards, 1),
            CardVar::new(Var::Energy, 1),
            CardVar::new(Var::Stars, 1),
            CardVar::new(Var::Forge, 5),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let card = eng.s.card(play.card);
        let cards = card.var(Var::Cards);
        let stars = card.var(Var::Stars);
        let energy = card.int_var(Var::Energy);
        let forge = card.var(Var::Forge);
        eng.draw(cards, false)?;
        eng.gain_stars(stars)?;
        eng.gain_energy(Decimal::from(energy))?;
        eng.forge(forge, Some(Listener::Card(play.card)))?;
        Ok(())
    }
}
