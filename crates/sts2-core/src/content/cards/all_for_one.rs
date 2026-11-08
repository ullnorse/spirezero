//! `AllForOne` (`Models.Cards/AllForOne.cs`): cost 2, 10 damage (14 upgraded), then every Attack, Skill or Power of the
//! discard pile that costs 0 (not X) goes to the hand, one at a time.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct AllForOne;

#[sts2_macros::hooks]
impl Hooks for AllForOne {
    type Me = CardId;
}

impl CardModel for AllForOne {
    fn name(&self) -> &'static str {
        "All for One"
    }

    fn cost(&self) -> i32 {
        2
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 10).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("AllForOne needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let hooked = eng.hooked();
        let free: Vec<CardId> = hooked
            .s
            .player
            .discard
            .iter()
            .copied()
            .filter(|&c| {
                let card = hooked.s.card(c);
                hooked.energy_cost(c) == 0
                    && !card.energy_cost.costs_x
                    && matches!(
                        card.card_type(),
                        CardType::Attack | CardType::Skill | CardType::Power
                    )
            })
            .collect();
        for card in free {
            eng.add_to_pile(card, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}
