//! `Eradicate` (`Models.Cards/Eradicate.cs`): cost X, Retain, X hits of 11 damage (14
//! upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Eradicate;

#[sts2_macros::hooks]
impl Hooks for Eradicate {
    type Me = CardId;
}

impl CardModel for Eradicate {
    fn name(&self) -> &'static str {
        "Eradicate"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn has_energy_cost_x(&self) -> bool {
        true
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Retain)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 11).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Eradicate needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        // A negative X (only from a ModifyXValue listener) means no hits, like the C# loop.
        let hits = u32::try_from(eng.hooked().resolve_energy_x_value(play.card)).unwrap_or(0);
        let player = eng.s.player.creature;
        let attack = Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        )
        .with_hits(hits);
        eng.attack(&attack)?;
        Ok(())
    }
}
