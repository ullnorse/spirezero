//! `KnowThyPlace` (`Models.Cards/KnowThyPlace.cs`): Regent common skill, cost 0, Exhaust (upgrade removes it); apply 1 Weak and 1 Vulnerable.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct KnowThyPlace;

#[sts2_macros::hooks]
impl Hooks for KnowThyPlace {
    type Me = CardId;
}

impl CardModel for KnowThyPlace {
    fn name(&self) -> &'static str {
        "Know Thy Place"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Weak), 1),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("KnowThyPlace needs a target");
        let player = eng.s.player.creature;
        for kind in [PowerKind::Weak, PowerKind::Vulnerable] {
            let amount = eng.s.card(play.card).var(Var::Power(kind));
            eng.apply_power_from_card(Some(play.card), target, kind, amount, Some(player))?;
        }
        Ok(())
    }
}
