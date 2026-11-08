//! `LeadingStrike` (`Models.Cards/LeadingStrike.cs`): cost 1, Attack, Common, Strike tag; 3
//! damage (6 upgraded), then creates 2 Shivs in the hand, one `Shiv.CreateInHand` each.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::shiv;
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct LeadingStrike;

#[sts2_macros::hooks]
impl Hooks for LeadingStrike {
    type Me = CardId;
}

impl CardModel for LeadingStrike {
    fn name(&self) -> &'static str {
        "Leading Strike"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Named("Shivs"), 2),
            CardVar::new(Var::Damage, 3).upgrade(3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Leading Strike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        for _ in 0..eng.s.card(play.card).int_var(Var::Named("Shivs")) {
            shiv::create_in_hand(eng, 1)?;
        }
        Ok(())
    }
}
