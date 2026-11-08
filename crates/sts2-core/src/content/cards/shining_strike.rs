//! `ShiningStrike` (`Models.Cards/ShiningStrike.cs`): Regent uncommon attack, cost 1; deal 8
//! damage (11 upgraded), then gain 2 stars. Tagged Strike. A play that would discard it puts it
//! on top of the draw pile instead.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ResultLocation, ResultPile};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct ShiningStrike;

#[sts2_macros::hooks]
impl Hooks for ShiningStrike {
    type Me = CardId;
}

impl CardModel for ShiningStrike {
    fn name(&self) -> &'static str {
        "Shining Strike"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn is_strike(&self) -> bool {
        true
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
            CardVar::new(Var::Damage, 8).upgrade(3),
            CardVar::new(Var::Stars, 2),
        ];
        VARS
    }

    fn result_location_for_card_play(&self, base: ResultLocation) -> ResultLocation {
        if base.pile == ResultPile::Discard {
            ResultLocation {
                pile: ResultPile::Draw,
                position: Position::Top,
            }
        } else {
            base
        }
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("ShiningStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.gain_stars(eng.s.card(play.card).var(Var::Stars))?;
        Ok(())
    }
}
