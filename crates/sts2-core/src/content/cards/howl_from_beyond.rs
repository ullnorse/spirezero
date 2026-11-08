//! `HowlFromBeyond` (`Models.Cards/HowlFromBeyond.cs`): cost 3; 18 damage (24 upgraded)
//! to all enemies. At the start of every auto post-play phase (the end of the turn) it plays
//! itself from the Exhaust pile (it has an Exhaust hover tip but no Exhaust keyword, so it only
//! gets there through other effects).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct HowlFromBeyond;

#[sts2_macros::hooks]
impl Hooks for HowlFromBeyond {
    type Me = CardId;

    fn after_auto_post_play_phase_entered(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        if eng.s.pile_of(me) == Some(PileType::Exhaust) {
            eng.auto_play(me, None)?;
        }
        Ok(())
    }
}

impl CardModel for HowlFromBeyond {
    fn name(&self) -> &'static str {
        "Howl from Beyond"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 18).upgrade(6)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
