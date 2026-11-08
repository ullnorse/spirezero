//! `Catastrophe` (`Models.Cards/Catastrophe.cs`): cost 2; autoplay 2 (3 upgraded) random draw-pile
//! cards, preferring playable ones. Each pick is the first card of a `StableShuffle` of the
//! candidates on `Rng.Shuffle`.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    self, CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::rng::RunRngType;
use crate::shuffle::stable_shuffle_by;

pub struct Catastrophe;

#[sts2_macros::hooks]
impl Hooks for Catastrophe {
    type Me = CardId;
}

impl CardModel for Catastrophe {
    fn name(&self) -> &'static str {
        "Catastrophe"
    }

    fn cost(&self) -> i32 {
        2
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

    fn may_prompt(&self) -> bool {
        // It autoplays arbitrary cards, some of which raise choices.
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        for _ in 0..eng.s.card(play.card).int_var(Var::Cards) {
            let hooked = eng.hooked();
            let playable: Vec<CardId> = hooked
                .s
                .player
                .draw
                .iter()
                .copied()
                .filter(|&c| !hooked.has_keyword(c, CardKeyword::Unplayable))
                .collect();
            let mut picked = first_of_stable_shuffle(eng, playable);
            if picked.is_none() {
                let all = eng.s.player.draw.to_vec();
                picked = first_of_stable_shuffle(eng, all);
            }
            if let Some(card) = picked {
                eng.auto_play(card, None)?;
            }
        }
        Ok(())
    }
}

/// `list.StableShuffle(Rng.Shuffle).FirstOrDefault()`.
fn first_of_stable_shuffle(eng: &mut Engine<'_>, mut list: Vec<CardId>) -> Option<CardId> {
    let state = &mut *eng.s;
    let all_cards = &state.cards;
    stable_shuffle_by(&mut list, &mut state.rng[RunRngType::Shuffle], |a, b| {
        cards::compare(&all_cards[a.index()], &all_cards[b.index()])
    });
    list.first().copied()
}
