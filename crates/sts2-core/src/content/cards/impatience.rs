//! `Impatience` (`Models.Cards/Impatience.cs`): cost 0; draw 2 (3 upgraded) if the hand holds no
//! Attack.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Impatience;

#[sts2_macros::hooks]
impl Hooks for Impatience {
    type Me = CardId;
}

impl CardModel for Impatience {
    fn name(&self) -> &'static str {
        "Impatience"
    }

    fn cost(&self) -> i32 {
        0
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 2).upgrade(1)];
        VARS
    }

    /// `ShouldGlowGoldInternal`: the hand holds no Attack.
    fn glows_gold(&self, s: &CombatState, _me: CardId) -> bool {
        !hand_has_attack(s)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).var(Var::Cards);
        if !hand_has_attack(eng.s) {
            eng.draw(cards, false)?;
        }
        Ok(())
    }
}

/// Whether the hand holds an Attack.
fn hand_has_attack(s: &CombatState) -> bool {
    s.player
        .hand
        .iter()
        .any(|&c| s.card(c).card_type() == CardType::Attack)
}
