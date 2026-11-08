//! `VoidForm` (`Models.Cards/VoidForm.cs`): Regent rare power, cost 3, Ethereal (removed on
//! upgrade); apply 2 `VoidFormPower`, then end the turn (`PlayerCmd.EndTurn`).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct VoidForm;

#[sts2_macros::hooks]
impl Hooks for VoidForm {
    type Me = CardId;
}

impl CardModel for VoidForm {
    fn name(&self) -> &'static str {
        "Void Form"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Ethereal)
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Ethereal);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::VoidForm), 2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::VoidForm,
            eng.s.card(play.card).var(Var::Power(PowerKind::VoidForm)),
            Some(player),
        )?;
        eng.end_turn();
        Ok(())
    }
}
