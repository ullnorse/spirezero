//! `MinionStrike` (`Models.Cards/MinionStrike.cs`): `Begone`'s token. Attack, cost 0, Token rarity,
//! Exhaust, tagged `CardTag.Strike` and `CardTag.Minion`; deals 6 damage (9 upgraded), then draws
//! 1 card.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct MinionStrike;

#[sts2_macros::hooks]
impl Hooks for MinionStrike {
    type Me = CardId;
}

impl CardModel for MinionStrike {
    fn name(&self) -> &'static str {
        "Minion Strike"
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn is_strike(&self) -> bool {
        true
    }

    fn is_minion(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 6).upgrade(3),
            CardVar::new(Var::Cards, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("MinionStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let cards = eng.s.card(play.card).var(Var::Cards);
        eng.draw(cards, false)?;
        Ok(())
    }
}
