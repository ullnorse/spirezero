//! `Peck` (`Models.Cards/Peck.cs`): event card, cost 1, 2 damage 3 times (4 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Peck;

#[sts2_macros::hooks]
impl Hooks for Peck {
    type Me = CardId;
}

impl CardModel for Peck {
    fn name(&self) -> &'static str {
        "Peck"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 2),
            CardVar::new(Var::Repeat, 3).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Peck needs a target");
        let hits = eng.s.card(play.card).count_var(Var::Repeat);
        let attacker = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                attacker,
                AttackTargets::Single(target),
                AttackDamage::Fixed(eng.s.card(play.card).var(Var::Damage)),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}
