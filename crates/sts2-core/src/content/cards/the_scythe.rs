//! `TheScythe` (`Models.Cards/TheScythe.cs`): rare attack, cost 2, Exhaust; deal 13 damage plus
//! every increase so far, then increase this card's damage by 5 (7 upgraded). The increase
//! survives a downgrade (`AfterDowngraded` recomputes the damage from it). A play also buffs the
//! card's deck version (`DeckVersion.BuffFromPlay`: the deck card's counter is its
//! `IncreasedDamage`), so the next fight's copy starts stronger.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{
    CardData, CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct TheScythe;

/// `_baseDamage`.
const BASE_DAMAGE: i32 = 13;

/// `TheScythe`'s fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `IncreasedDamage`.
    pub increased_damage: i32,
    /// `CurrentDamage`, the `DamageVar`'s base value.
    pub current_damage: i32,
}

fn data(card: &Card) -> Data {
    match card.data {
        CardData::TheScythe(data) => data,
        CardData::None | CardData::Bolas(_) | CardData::Rampage(_) => Data {
            increased_damage: 0,
            current_damage: BASE_DAMAGE,
        },
        _ => unreachable!("a TheScythe holds TheScythe data"),
    }
}

/// The saved fields of a deck card whose `IncreasedDamage` is `counter`, which the combat clone
/// copies.
pub fn data_from_deck(counter: u32) -> CardData {
    let increased_damage = i32::try_from(counter).expect("IncreasedDamage fits i32");
    CardData::TheScythe(Data {
        increased_damage,
        current_damage: BASE_DAMAGE + increased_damage,
    })
}

/// `UpdateDamage`: `CurrentDamage = 13 + IncreasedDamage`.
fn update_damage(s: &mut CombatState, me: CardId, increased_damage: i32) {
    s.card_mut(me).data = CardData::TheScythe(Data {
        increased_damage,
        current_damage: BASE_DAMAGE + increased_damage,
    });
}

#[sts2_macros::hooks]
impl Hooks for TheScythe {
    type Me = CardId;
}

impl CardModel for TheScythe {
    fn name(&self) -> &'static str {
        "The Scythe"
    }

    fn cost(&self) -> i32 {
        2
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, BASE_DAMAGE),
            CardVar::new(Var::Named("Increase"), 5).upgrade(2),
        ];
        VARS
    }

    fn instance_var(&self, card: &Card, name: Var, value: Decimal) -> Decimal {
        match name {
            Var::Damage => Decimal::from(data(card).current_damage),
            _ => value,
        }
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("TheScythe needs a target");
        let player = eng.s.player.creature;
        let damage = eng.s.card(play.card).var(Var::Damage);
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // BuffFromPlay.
        let increase = eng.s.card(play.card).count_var(Var::Named("Increase"));
        let increased = data(eng.s.card(play.card)).increased_damage + increase.cast_signed();
        update_damage(eng.s, play.card, increased);
        // `(DeckVersion as TheScythe)?.BuffFromPlay(increase)`.
        if let Some(i) = eng.s.card(play.card).deck_version() {
            eng.s.deck[i].counter += increase;
        }
        Ok(())
    }

    fn after_downgraded(&self, s: &mut CombatState, me: CardId) {
        let increased = data(s.card(me)).increased_damage;
        update_damage(s, me, increased);
    }
}
