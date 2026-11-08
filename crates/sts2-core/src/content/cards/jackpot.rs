//! `Jackpot` (`Models.Cards/Jackpot.cs`): cost 3; 25 damage (30 upgraded), then 3 random cards of
//! the character's pool that cost 0 (and are not X cost) go to the hand, upgraded if Jackpot is.
//! `CardFactory.GetForCombat` may pick the same card twice.
//!
//! The pool's unported cards (`CallOfTheVoid`, `SentryMode`) cost 1 and 2, so none can be picked.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var, pools};

pub struct Jackpot;

#[sts2_macros::hooks]
impl Hooks for Jackpot {
    type Me = CardId;
}

impl CardModel for Jackpot {
    fn name(&self) -> &'static str {
        "Jackpot"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 25).upgrade(5),
            CardVar::new(Var::Cards, 3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Jackpot needs a target");
        let upgraded = eng.s.card(play.card).is_upgraded();
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // `Canonical == 0 && !CostsX`.
        let character = eng.s.player.character;
        let zero_cost: Vec<_> = pools::character(character)
            .into_iter()
            .filter(|c| character.zero_cost_cards().contains(&c.id))
            .collect();
        let count = eng.s.card(play.card).count_var_len(Var::Cards);
        let cards = pools::get_for_combat(eng, &zero_cost, count)?;
        for card in cards {
            if upgraded {
                eng.upgrade_card(card);
            }
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
