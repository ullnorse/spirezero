//! `Bombardment` (`Models.Cards/Bombardment.cs`): Regent rare attack, cost 3, Exhaust; deal 18
//! damage (24 upgraded). Once exhausted it plays itself (no cost, random target) every time the
//! auto-play phase starts, `AfterAutoPrePlayPhaseEnteredEarly`.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Bombardment;

#[sts2_macros::hooks]
impl Hooks for Bombardment {
    type Me = CardId;

    fn after_auto_pre_play_phase_entered_early(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        // `player == Owner`: the one player.
        if eng.s.pile_of(me) == Some(PileType::Exhaust) {
            eng.auto_play(me, None)?;
        }
        Ok(())
    }
}

impl CardModel for Bombardment {
    fn name(&self) -> &'static str {
        "Bombardment"
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 18).upgrade(6)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Bombardment needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
