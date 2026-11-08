//! `PactsEnd` (`Models.Cards/PactsEnd.cs`): cost 0; with at least 3 cards in the exhaust pile, 18 damage (24 upgraded) to all enemies.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct PactsEnd;

#[sts2_macros::hooks]
impl Hooks for PactsEnd {
    type Me = CardId;
}

impl CardModel for PactsEnd {
    fn name(&self) -> &'static str {
        "Pact's End"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 18).upgrade(6),
            CardVar::new(Var::Cards, 3),
        ];
        VARS
    }

    /// `ShouldGlowGoldInternal`: `CanDealDamage`.
    fn glows_gold(&self, s: &CombatState, me: CardId) -> bool {
        can_deal_damage(s, me)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        if !can_deal_damage(eng.s, play.card) {
            return Ok(());
        }
        let damage = eng.s.card(play.card).var(Var::Damage);
        eng.attack(&Attack::new(
            player,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}

/// `CanDealDamage`: at least `Cards` cards in the exhaust pile.
fn can_deal_damage(s: &CombatState, me: CardId) -> bool {
    s.player.exhaust.len() >= s.card(me).count_var_len(Var::Cards)
}
