//! `Voltaic` (`Models.Cards/Voltaic.cs`): cost 3, Exhaust (upgrade removes it); channel Lightning once per Lightning orb channelled this combat.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, OrbKind};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct Voltaic;

#[sts2_macros::hooks]
impl Hooks for Voltaic {
    type Me = CardId;
}

impl CardModel for Voltaic {
    fn name(&self) -> &'static str {
        "Voltaic"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::Named("CalculatedChannels"), lightning_channelled),
        ];
        VARS
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let count = eng
            .s
            .calculate_int(play.card, Var::Named("CalculatedChannels"), play.target);
        for _ in 0..count {
            eng.channel(OrbKind::Lightning)?;
        }
        Ok(())
    }
}

/// The Lightning orbs the owner channelled this combat (`OrbChanneledEntry`).
fn lightning_channelled(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    Decimal::from(i32::try_from(s.orbs_channeled_this_combat(OrbKind::Lightning)).expect("a few"))
}
