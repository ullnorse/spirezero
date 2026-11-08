//! `BeatDown` (`Models.Cards/BeatDown.cs`): skill, cost 3; auto-play 3 (4 upgraded) random
//! playable Attacks of the discard pile (`StableShuffle` on `Rng.Shuffle`), each `AnyEnemy` one
//! at a random hittable enemy (`Rng.CombatTargets`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    self, CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::rng::RunRngType;
use crate::shuffle::stable_shuffle_by;

pub struct BeatDown;

#[sts2_macros::hooks]
impl Hooks for BeatDown {
    type Me = CardId;
}

impl CardModel for BeatDown {
    /// Auto-plays cards from the discard pile; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Beat Down"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::RandomEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let count = eng.s.card(play.card).count_var_len(Var::Cards);
        let hooked = eng.hooked();
        let mut attacks: Vec<CardId> = hooked
            .s
            .player
            .discard
            .iter()
            .copied()
            .filter(|&c| {
                hooked.s.card(c).card_type() == CardType::Attack
                    && !hooked.has_keyword(c, CardKeyword::Unplayable)
            })
            .collect();
        let state = &mut *eng.s;
        let all_cards = &state.cards;
        stable_shuffle_by(&mut attacks, &mut state.rng[RunRngType::Shuffle], |a, b| {
            cards::compare(&all_cards[a.index()], &all_cards[b.index()])
        });
        attacks.truncate(count);
        for card in attacks {
            if eng.hooked().is_over_or_ending() {
                break;
            }
            let target = if eng.s.target_type(card) == TargetType::AnyEnemy {
                let hittable = eng.hooked().hittable_enemies();
                eng.s.rng[RunRngType::CombatTargets]
                    .next_item(&hittable)
                    .copied()
            } else {
                None
            };
            eng.auto_play(card, target)?;
        }
        Ok(())
    }
}
