//! `KnifeTrap` (`Models.Cards/KnifeTrap.cs`): cost 2, Skill, Rare; auto-plays every Shiv in the
//! Exhaust pile at the target (upgrading each first when this card is upgraded). The Shivs
//! exhaust again as they are played.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct KnifeTrap;

#[sts2_macros::hooks]
impl Hooks for KnifeTrap {
    type Me = CardId;
}

impl CardModel for KnifeTrap {
    /// Auto-plays the Shivs of the exhaust pile; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Knife Trap"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::CalculationBase, 0),
            CardVar::new(Var::CalculationExtra, 1),
            CardVar::calculated(Var::Named("CalculatedShivs"), shivs_in_exhaust_pile),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Knife Trap needs a target");
        let upgraded = eng.s.card(play.card).is_upgraded();
        let shivs: Vec<CardId> = eng
            .s
            .player
            .exhaust
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).kind.is_shiv())
            .collect();
        for card in shivs {
            if upgraded {
                eng.upgrade_card(card);
            }
            eng.auto_play(card, Some(target))?;
        }
        Ok(())
    }
}

/// The Shiv-tagged cards in the owner's exhaust pile (the text's `CalculatedShivs`).
fn shivs_in_exhaust_pile(s: &CombatState, _card: CardId, _target: Option<CreatureId>) -> Decimal {
    let count = s
        .player
        .exhaust
        .iter()
        .filter(|&&c| s.card(c).kind.is_shiv())
        .count();
    Decimal::from(i32::try_from(count).expect("a combat has few cards"))
}
