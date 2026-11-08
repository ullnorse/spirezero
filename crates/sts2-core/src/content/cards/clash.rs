//! `Clash` (`Models.Cards/Clash.cs`): Event attack, cost 0; 14 damage (18 upgraded), only
//! playable when every card in the hand is an Attack.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Clash;

#[sts2_macros::hooks]
impl Hooks for Clash {
    type Me = CardId;
}

impl CardModel for Clash {
    fn name(&self) -> &'static str {
        "Clash"
    }

    fn cost(&self) -> i32 {
        0
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 14).upgrade(4)];
        VARS
    }

    fn is_playable(&self, s: &CombatState, _me: CardId) -> bool {
        s.player
            .hand
            .iter()
            .all(|&c| s.card(c).card_type() == CardType::Attack)
    }

    /// `ShouldGlowGoldInternal`: `IsPlayable`.
    fn glows_gold(&self, s: &CombatState, me: CardId) -> bool {
        self.is_playable(s, me)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Clash needs a target");
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
