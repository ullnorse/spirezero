//! `GrandFinale` (`Models.Cards/GrandFinale.cs`): cost 0, playable only with an empty draw pile: 60 damage (75 upgraded) to all enemies.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct GrandFinale;

#[sts2_macros::hooks]
impl Hooks for GrandFinale {
    type Me = CardId;
}

impl CardModel for GrandFinale {
    fn name(&self) -> &'static str {
        "Grand Finale"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn is_playable(&self, s: &CombatState, _me: CardId) -> bool {
        s.player.draw.is_empty()
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 60).upgrade(15)];
        VARS
    }

    /// `ShouldGlowGoldInternal`: `IsPlayable`.
    fn glows_gold(&self, s: &CombatState, me: CardId) -> bool {
        self.is_playable(s, me)
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
