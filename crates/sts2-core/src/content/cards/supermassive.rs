//! `Supermassive` (`Models.Cards/Supermassive.cs`): Regent uncommon attack, cost 1; deal 5 damage
//! plus 3 (4 upgraded) for each card the owner generated this combat (`CardGeneratedEntry`s whose
//! `Creator` is the owner).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Supermassive;

#[sts2_macros::hooks]
impl Hooks for Supermassive {
    type Me = CardId;
}

impl CardModel for Supermassive {
    fn name(&self) -> &'static str {
        "Supermassive"
    }

    fn cost(&self) -> i32 {
        1
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 5),
            CardVar::new(Var::ExtraDamage, 3).upgrade(1),
            CardVar::calculated(Var::CalculatedDamage, cards_generated),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Supermassive needs a target");
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Calculated(play.card),
            Some(play.card),
        ))?;
        Ok(())
    }
}

/// `History.Entries.OfType<CardGeneratedEntry>().Count(c => c.Creator == card.Owner)`.
fn cards_generated(state: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let count = state.cards_generated_by_player_this_combat();
    Decimal::from(i32::try_from(count).expect("a combat generates few cards"))
}
