//! `Stardust` (`Models.Cards/Stardust.cs`): Regent uncommon attack, cost 0 and X stars; deal 5
//! damage (7 upgraded) to a random enemy X times (`ResolveStarXValue`).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Stardust;

#[sts2_macros::hooks]
impl Hooks for Stardust {
    type Me = CardId;
}

impl CardModel for Stardust {
    fn name(&self) -> &'static str {
        "Stardust"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn has_star_cost_x(&self) -> bool {
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 5).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let hits = u32::try_from(eng.hooked().resolve_star_x_value(play.card)).unwrap_or(0);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::RandomOpponents,
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}
