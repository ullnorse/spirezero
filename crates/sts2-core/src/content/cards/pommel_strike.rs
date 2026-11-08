//! `PommelStrike` (`Models.Cards/PommelStrike.cs`): cost 1, Strike; 9 damage (10 upgraded), then draws 1 card (2 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct PommelStrike;

#[sts2_macros::hooks]
impl Hooks for PommelStrike {
    type Me = CardId;
}

impl CardModel for PommelStrike {
    fn name(&self) -> &'static str {
        "Pommel Strike"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 9).upgrade(1),
            CardVar::new(Var::Cards, 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("PommelStrike needs a target");
        let player = eng.s.player.creature;
        let damage = eng.s.card(play.card).var(Var::Damage);
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        Ok(())
    }
}
