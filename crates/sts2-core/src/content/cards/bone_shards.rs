//! `BoneShards` (`Models.Cards/BoneShards.cs`): cost 1, Osty hits all enemies for 9 (12
//! upgraded), the player gains 9 Block (12), then Osty dies. Does nothing without a living Osty.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct BoneShards;

#[sts2_macros::hooks]
impl Hooks for BoneShards {
    type Me = CardId;
}

impl CardModel for BoneShards {
    fn name(&self) -> &'static str {
        "Bone Shards"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
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

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::OstyDamage, 9).upgrade(3),
            CardVar::new(Var::Block, 9).upgrade(3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // Osty.CheckMissingWithAnim.
        if !eng.s.is_osty_alive() {
            return Ok(());
        }
        let damage = eng.s.card(play.card).var(Var::OstyDamage);
        let osty = eng.s.osty().expect("Osty is alive");
        eng.attack(&Attack::new(
            osty,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let player = eng.s.player.creature;
        let block = eng.s.card(play.card).var(Var::Block);
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        if eng.s.is_osty_alive() {
            eng.kill(&[osty])?;
        }
        Ok(())
    }
}
