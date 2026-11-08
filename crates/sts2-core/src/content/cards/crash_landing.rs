//! `CrashLanding` (`Models.Cards/CrashLanding.cs`): Regent rare attack, cost 1, all enemies; deal
//! 21 damage (26 upgraded), then fill the hand with `Debris`.

use crate::combat::commands::MAX_HAND_SIZE;
use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct CrashLanding;

#[sts2_macros::hooks]
impl Hooks for CrashLanding {
    type Me = CardId;
}

impl CardModel for CrashLanding {
    fn name(&self) -> &'static str {
        "Crash Landing"
    }

    fn cost(&self) -> i32 {
        1
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 21).upgrade(5)];
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
        let missing = MAX_HAND_SIZE.saturating_sub(eng.s.player.hand.len());
        let debris: Vec<CardId> = (0..missing)
            .map(|_| eng.create_card(CardKind::Debris))
            .collect();
        eng.add_generated_cards_to_combat(&debris, PileType::Hand, Some(player), Position::Bottom)
    }
}
