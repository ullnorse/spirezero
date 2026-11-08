//! `MindBlast` (`Models.Cards/MindBlast.cs`): cost 1 (0 upgraded), Innate; damage equal to the
//! number of cards in the draw pile (`0 + 1 ×`).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct MindBlast;

#[sts2_macros::hooks]
impl Hooks for MindBlast {
    type Me = CardId;
}

impl CardModel for MindBlast {
    fn name(&self) -> &'static str {
        "Mind Blast"
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Innate)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::ExtraDamage, 1),
            CardVar::calculated(Var::CalculatedDamage, draw_pile_size),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("MindBlast needs a target");
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

/// `PileType.Draw.GetPile(card.Owner).Cards.Count`.
fn draw_pile_size(state: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(i32::try_from(state.player.draw.len()).expect("a pile fits an i32"))
}
