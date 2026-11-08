//! `Scrape` (`Models.Cards/Scrape.cs`): Defect uncommon attack, cost 1; 7 damage (10 upgraded),
//! draw 4 cards (5 upgraded), then discard the drawn cards whose cost (with every modifier) is
//! not 0; an X-cost card always counts as non-zero.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Scrape;

#[sts2_macros::hooks]
impl Hooks for Scrape {
    type Me = CardId;
}

impl CardModel for Scrape {
    fn name(&self) -> &'static str {
        "Scrape"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 7).upgrade(3),
            CardVar::new(Var::Cards, 4).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Scrape needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let drawn = eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        // EnergyCost.GetWithModifiers(All) != 0 || EnergyCost.CostsX.
        let cards: Vec<CardId> = drawn
            .into_iter()
            .filter(|&c| eng.hooked().energy_cost(c) != 0 || eng.s.card(c).energy_cost.costs_x)
            .collect();
        eng.discard(&cards)
    }
}
