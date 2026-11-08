//! `Compact` (`Models.Cards/Compact.cs`): cost 1, 6 block (7 upgraded); every Status card of the hand becomes a `Fuel`
//! (upgraded when `Compact` is), with one `CardCmd.Transform` for the whole list.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, HistoryKind, ValueProps};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Compact;

#[sts2_macros::hooks]
impl Hooks for Compact {
    type Me = CardId;
}

impl CardModel for Compact {
    fn name(&self) -> &'static str {
        "Compact"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 6).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let upgraded = eng.s.card(play.card).is_upgraded();
        // `c.IsTransformable`: only an Eternal card of the deck fails it.
        let statuses: Vec<CardId> = eng
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).card_type() == CardType::Status)
            .collect();
        let mut transformations = Vec::new();
        for status in statuses {
            let fuel = eng.create_card(CardKind::Fuel);
            if upgraded {
                eng.upgrade_card(fuel);
            }
            transformations.push((status, fuel));
        }
        transform(eng, &transformations)
    }
}

/// `CardCmd.Transform(transformations, null)` for cards of the hand (`CardCmd.cs:373-455`): nothing
/// once combat is ending; else every original leaves its pile first, remembering its index at
/// that moment, then the replacements are added in ascending index order (a stable sort, since a
/// hand is far below the 17 elements where `List.Sort` stops using insertion sort), each at its
/// recorded index, so several transformed cards end up closer together than they were. Then
/// `AfterCardGeneratedForCombat` for each.
fn transform(eng: &mut Engine<'_>, transformations: &[(CardId, CardId)]) -> Flow {
    if eng.hooked().is_ending() || transformations.is_empty() {
        return Ok(());
    }
    let mut removed = Vec::with_capacity(transformations.len());
    for &(original, replacement) in transformations {
        let pile = eng
            .s
            .pile_of(original)
            .expect("a transformed card is in a pile");
        let index = eng
            .s
            .player
            .pile(pile)
            .iter()
            .position(|&c| c == original)
            .expect("the card is in its pile");
        eng.s.remove_from_pile(original);
        removed.push((pile, index, replacement));
    }
    removed.sort_by_key(|&(_, index, _)| index);
    let me = eng.s.player.creature;
    for &(pile, index, replacement) in &removed {
        eng.s.insert_into_pile(replacement, pile, Some(index));
        eng.record(
            me,
            HistoryKind::CardGenerated {
                card: replacement,
                creator: Some(me),
            },
        );
        eng.after_card_entered_combat(replacement)?;
        // The old pile passed is the replacement's own pile (`CardCmd.cs:451`).
        eng.after_card_changed_piles(replacement, Some(pile))?;
    }
    for &(_, _, replacement) in &removed {
        if eng.s.pile_of(replacement).is_some() {
            eng.after_card_generated_for_combat(replacement, Some(me))?;
        }
    }
    Ok(())
}
