//! `ToastyMittens` (`Models.Relics/ToastyMittens.cs`): Ancient (Tezcatara); after each of the
//! owner's hand draws, it exhausts 1 hand card of its choice (`FromHand`), then gains 1 Strength.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, Prompt, PromptText, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `CardSelectorPrefs(ExhaustSelectionPrompt, 1)`.
const CARDS: usize = 1;
/// `PowerVar<StrengthPower>(1m)`.
const STRENGTH: Decimal = Decimal::ONE;

pub struct ToastyMittens;

impl RelicModel for ToastyMittens {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Toasty Mittens"
    }
}

#[sts2_macros::hooks]
impl Hooks for ToastyMittens {
    type Me = RelicId;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        // `player != Owner.Creature.Player`: the only player.
        let options = eng.s.player.hand.to_vec();
        for card in eng.choose_cards(
            Prompt::relic(me, PromptText::Exhaust),
            options,
            CARDS,
            CARDS,
        )? {
            eng.exhaust(card, false)?;
        }
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Strength, STRENGTH, Some(player))
    }
}
