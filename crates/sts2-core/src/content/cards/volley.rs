//! `Volley` (`Models.Cards/Volley.cs`): attack, X cost; X hits of 10 (14 upgraded), each on a
//! random opponent.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Volley;

#[sts2_macros::hooks]
impl Hooks for Volley {
    type Me = CardId;
}

impl CardModel for Volley {
    fn name(&self) -> &'static str {
        "Volley"
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
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::RandomEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 10).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        // A negative X (only from a ModifyXValue listener) means no hits, like the C# loop.
        let hits = u32::try_from(eng.hooked().resolve_energy_x_value(play.card)).unwrap_or(0);
        let player = eng.s.player.creature;
        let attack = Attack::new(
            player,
            AttackTargets::RandomOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        )
        .with_hits(hits);
        eng.attack(&attack)?;
        Ok(())
    }
}
