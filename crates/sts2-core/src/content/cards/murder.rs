//! `Murder` (`Models.Cards/Murder.cs`): cost 3 (2 upgraded), 1 damage plus 1 per card drawn this combat.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, HistoryTag};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Murder;

#[sts2_macros::hooks]
impl Hooks for Murder {
    type Me = CardId;
}

impl CardModel for Murder {
    fn name(&self) -> &'static str {
        "Murder"
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

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 1),
            CardVar::new(Var::ExtraDamage, 1),
            CardVar::calculated(Var::CalculatedDamage, cards_drawn_this_combat),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Murder needs a target");
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

/// `CardDrawnEntry`s over the whole combat with the owner as actor.
fn cards_drawn_this_combat(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(i32::try_from(s.history_count(HistoryTag::CardDrawn)).expect("count fits i32"))
}
