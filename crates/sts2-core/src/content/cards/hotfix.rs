//! `Hotfix` (`Models.Cards/Hotfix.cs`): cost 0 Skill, Exhaust (removed upgraded); 2 Focus until
//! the end of the turn (`HotfixPower`).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Hotfix;

#[sts2_macros::hooks]
impl Hooks for Hotfix {
    type Me = CardId;
}

impl CardModel for Hotfix {
    fn name(&self) -> &'static str {
        "Hotfix"
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
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Focus), 2)];
        VARS
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let focus = eng.s.card(play.card).int_var(Var::Power(PowerKind::Focus));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Hotfix,
            Decimal::from(focus),
            Some(player),
        )
    }
}
