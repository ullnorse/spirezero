//! `Severance` (`Models.Cards/Severance.cs`): cost 2, 13 damage (18 upgraded), then adds a new
//! Soul to the draw pile at a random position, one to the discard pile and one to the hand.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Severance;

#[sts2_macros::hooks]
impl Hooks for Severance {
    type Me = CardId;
}

impl CardModel for Severance {
    fn name(&self) -> &'static str {
        "Severance"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 13).upgrade(5)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Severance needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // Soul.Create(Owner, 3, ..): all three exist before the first is added.
        let souls: Vec<CardId> = (0..3).map(|_| eng.create_card(CardKind::Soul)).collect();
        eng.add_generated_card_to_combat_at(
            souls[0],
            PileType::Draw,
            Some(player),
            Position::Random,
        )?;
        eng.add_generated_card_to_combat(souls[1], PileType::Discard, Some(player))?;
        eng.add_generated_card_to_combat(souls[2], PileType::Hand, Some(player))
    }
}
