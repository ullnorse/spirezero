//! `BladeOfInk` (`Models.Cards/BladeOfInk.cs`): cost 1, Skill, Rare; creates 2 Shivs in the hand
//! (3 upgraded) with one `Shiv.CreateInHand`, and enchants each with `Inky`.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::shiv;
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::enchantments::EnchantmentKind;
use crate::decimal::Decimal;

pub struct BladeOfInk;

#[sts2_macros::hooks]
impl Hooks for BladeOfInk {
    type Me = CardId;
}

impl CardModel for BladeOfInk {
    fn name(&self) -> &'static str {
        "Blade of Ink"
    }

    fn cost(&self) -> i32 {
        1
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).count_var_len(Var::Cards);
        for shiv in shiv::create_in_hand(eng, cards)? {
            eng.enchant(shiv, EnchantmentKind::Inky, Decimal::ONE);
        }
        Ok(())
    }
}
