//! `Shiv` (`Models.Cards/Shiv.cs`): the Silent's token. Attack, cost 0, Token rarity, Exhaust,
//! tagged `CardTag.Shiv`; 4 damage (6 upgraded). While its owner has `FanOfKnivesPower` it
//! targets and hits every enemy. [`create_in_hand`] is `Shiv.CreateInHand`.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, PileType};
use crate::content::cards::{
    CardKeyword, CardKind, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Shiv;

/// `HasFanOfKnives`: the card is mutable (a combat card) and its owner has `FanOfKnivesPower`.
fn has_fan_of_knives(s: &CombatState) -> bool {
    s.creature(s.player.creature)
        .has_power(PowerKind::FanOfKnives)
}

/// `Shiv.CreateInHand(owner, count, combatState)`: `count` new Shivs added to the hand (nothing
/// for 0, or once the combat is over or ending). Returns the cards so callers can upgrade them.
pub fn create_in_hand(eng: &mut Engine<'_>, count: usize) -> Flow<Vec<CardId>> {
    if count == 0 || eng.hooked().is_over_or_ending() {
        return Ok(Vec::new());
    }
    let shivs: Vec<CardId> = (0..count)
        .map(|_| eng.create_card(CardKind::Shiv))
        .collect();
    let owner = eng.s.player.creature;
    eng.add_generated_cards_to_combat(&shivs, PileType::Hand, Some(owner), Position::Bottom)?;
    Ok(shivs)
}

#[sts2_macros::hooks]
impl Hooks for Shiv {
    type Me = CardId;
}

impl CardModel for Shiv {
    fn name(&self) -> &'static str {
        "Shiv"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Token
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn target_in(&self, s: &CombatState, _me: CardId) -> TargetType {
        if has_fan_of_knives(s) {
            TargetType::AllEnemies
        } else {
            TargetType::AnyEnemy
        }
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn is_shiv(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 4).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let targets = if has_fan_of_knives(eng.s) {
            AttackTargets::AllOpponents
        } else {
            AttackTargets::Single(play.target.expect("Shiv needs a target"))
        };
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            targets,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
