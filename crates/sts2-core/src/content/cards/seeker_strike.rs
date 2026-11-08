//! `SeekerStrike` (`Models.Cards/SeekerStrike.cs`): cost 1; 9 damage (12 upgraded), then choose
//! one of 3 random draw pile cards and put it in the hand. Tagged `Strike` (read by no ported
//! card).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{self, CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::rng::RunRngType;
use crate::shuffle::stable_shuffle_by;

pub struct SeekerStrike;

#[sts2_macros::hooks]
impl Hooks for SeekerStrike {
    type Me = CardId;
}

impl CardModel for SeekerStrike {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Seeker Strike"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 9).upgrade(3),
            CardVar::new(Var::Cards, 3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("SeekerStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // Draw.Cards.ToList().StableShuffle(Rng.CombatCardSelection).Take(Cards): the shuffled
        // list is built once, so the rng advances once.
        let count = eng.s.card(play.card).count_var_len(Var::Cards);
        let mut shuffled = eng.s.player.draw.to_vec();
        let state = &mut *eng.s;
        let all_cards = &state.cards;
        stable_shuffle_by(
            &mut shuffled,
            &mut state.rng[RunRngType::CombatCardSelection],
            |a, b| cards::compare(&all_cards[a.index()], &all_cards[b.index()]),
        );
        shuffled.truncate(count);
        // CardSelectCmd.FromCombatPile(Draw, prefs 1, c => cardOptions.Contains(c)): the pile's
        // own order.
        let options: Vec<CardId> = eng
            .s
            .player
            .draw
            .iter()
            .copied()
            .filter(|c| shuffled.contains(c))
            .collect();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        {
            eng.add_to_pile(card, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}
