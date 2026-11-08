//! `TheBomb` (`Models.Cards/TheBomb.cs`): skill, cost 2; apply a new `TheBombPower` instance of
//! 3 turns that deals 40 damage (50 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::{PowerData, PowerKind, the_bomb};

pub struct TheBomb;

#[sts2_macros::hooks]
impl Hooks for TheBomb {
    type Me = CardId;
}

impl CardModel for TheBomb {
    fn name(&self) -> &'static str {
        "The Bomb"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Named("Turns"), 3),
            CardVar::new(Var::Named("BombDamage"), 40).upgrade(10),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Named("BombDamage"));
        let turns = eng.s.card(play.card).var(Var::Named("Turns"));
        let player = eng.s.player.creature;
        let power = eng.apply_power_instance(player, PowerKind::TheBomb, turns, Some(player))?;
        // `.SetDamage(BombDamage)` on the returned instance.
        if let Some(me) = power {
            eng.s
                .set_power_data(me.id, PowerData::TheBomb(the_bomb::Data { damage }));
        }
        Ok(())
    }
}
