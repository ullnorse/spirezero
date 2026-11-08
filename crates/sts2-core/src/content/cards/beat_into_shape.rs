//! `BeatIntoShape` (`Models.Cards/BeatIntoShape.cs`): Regent rare attack, cost 1, any enemy; deal
//! 5 damage (7 upgraded), then Forge `CalculatedForge - hits * CalculationExtra`, where
//! `CalculatedForge` is `5 + 5 * (powered attacks the owner dealt the target this turn)` (7 + 7
//! upgraded): the forge counts only the hits from before this card.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::{Hooks, Listener};
use crate::combat::{
    CardId, CardPlay, CombatState, CreatureId, Engine, Flow, HistoryKind, HistoryTag,
};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct BeatIntoShape;

/// `CalculatedVar("CalculatedForge")`.
const CALCULATED_FORGE: Var = Var::Named("CalculatedForge");

#[sts2_macros::hooks]
impl Hooks for BeatIntoShape {
    type Me = CardId;
}

impl CardModel for BeatIntoShape {
    fn name(&self) -> &'static str {
        "Beat Into Shape"
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
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 5).upgrade(2),
            CardVar::new(Var::CalculationBase, 5).upgrade(2),
            CardVar::new(Var::CalculationExtra, 5).upgrade(2),
            CardVar::calculated(CALCULATED_FORGE, powered_hits_this_turn),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("BeatIntoShape needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        let results = eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let calculated = eng
            .s
            .calculate(play.card, CALCULATED_FORGE, Some(target))
            .expect("BeatIntoShape has a CalculatedForge var");
        let extra = eng.s.card(play.card).var(Var::CalculationExtra);
        let hits = Decimal::from(i32::try_from(results.len()).expect("hit count fits i32"));
        eng.forge(calculated - hits * extra, Some(Listener::Card(play.card)))?;
        Ok(())
    }
}

/// The multiplier: the `DamageReceivedEntry`s this turn with the target as receiver, the owner
/// as dealer and a powered attack.
fn powered_hits_this_turn(s: &CombatState, _card: CardId, target: Option<CreatureId>) -> Decimal {
    let player = s.player.creature;
    let count = s
        .history_this_turn(HistoryTag::DamageReceived)
        .iter()
        .filter(|e| Some(e.actor) == target)
        .filter(|e| {
            matches!(&e.kind, HistoryKind::DamageReceived { props, dealer, .. }
                if *dealer == Some(player) && props.is_powered_attack())
        })
        .count();
    Decimal::from(i32::try_from(count).expect("entry count fits i32"))
}
