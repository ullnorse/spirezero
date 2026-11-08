//! The hand as a list of rows, one per card: its number, cost, name and its whole text on one
//! line, the numbers Strength or Weak changed lit;
//! a card that cannot be played dim, the selected one highlighted, the focused one (and the one
//! waiting for its target) with its text in full. A card choice of the fight
//! offers its cards the same way, with a Done button for the End Turn button's place.

use sts2_core::combat::CardId;
use sts2_view::model::ButtonState;
use sts2_view::ui::ids::{self, Hit};

use super::tiles::Tile;
use super::{Big, View};
use crate::hit::{Target, UiAction};
use crate::text;
use crate::ui::button::Button;
use crate::ui::cards;
use crate::ui::scale;

/// The hand's main button: Done in a card choice that has one, else End Turn (dim outside the
/// player's turn).
#[derive(Clone)]
pub struct Main {
    pub key: &'static str,
    pub label: &'static str,
    pub action: UiAction,
    pub enabled: bool,
}

#[must_use]
pub fn main_button(view: &View<'_>) -> Option<Main> {
    let main = view.main?;
    if view.choice.is_some() {
        let Hit::Row(at) = ids::decode(main.id) else {
            return None;
        };
        return Some(Main {
            key: KEYS.get(at).copied().unwrap_or("Enter"),
            label: "Done",
            action: UiAction::Pick(at + 1),
            enabled: true,
        });
    }
    Some(Main {
        key: "e",
        label: "End Turn",
        action: UiAction::EndTurn,
        enabled: view.in_turn(),
    })
}

/// The keys of the first ten picks.
const KEYS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

/// The hand (or the card choice's cards) as buttons: `1  ϟ1 Strike   Deal 6 damage`; on a
/// big screen (`big.tall_cards`) two rows each, the name bold on the first and the text on
/// the second.
#[must_use]
pub fn buttons(view: &View<'_>, big: Big) -> Vec<Button> {
    let faces: Vec<_> = view
        .cards
        .iter()
        .filter_map(|row| {
            let Hit::Row(at) = ids::decode(row.id) else {
                return None;
            };
            Some((row, row.card.as_ref()?, at))
        })
        .collect();
    let rows: Vec<cards::Row> = faces
        .iter()
        .map(|(_, face, _)| cards::Row {
            cost: face.cost.clone(),
            name: face.name.clone(),
            kind: face.kind.clone(),
            rarity: face.rarity.clone(),
            effect: text::spans(&face.effect),
            text: text::spans(&face.text),
            glow: face.glow,
        })
        .collect();
    faces
        .iter()
        .zip(cards::spans(&rows, big.tall_cards))
        .map(|((row, face, at), (line, expanded))| {
            let choice = view.choice.is_some();
            let selected = !choice && row.state == ButtonState::Selected;
            let dim = !choice && matches!(row.state, ButtonState::Dim(_));
            let target = if choice {
                Target::Choice(at + 1, CardId(face.card))
            } else {
                Target::Card(CardId(face.card))
            };
            let key = KEYS
                .get(*at)
                .map_or_else(|| (at + 1).to_string(), |k| (*k).to_owned());
            let button = Button::new(key, line, Some(target))
                .dim(dim)
                .selected(selected)
                .more(true)
                .expanded(expanded)
                .open(scale::wide() || big.tall_cards);
            // A big screen's two-row cards are a band already: padded only with rows to spare.
            if big.tall_cards {
                button.pad(big.pad_cards)
            } else {
                button
            }
        })
        .collect()
}

/// The hand as card tiles (a big screen's, [`super::tiles`]): each card's key, cost, name, type
/// and whole text, with the state the list rows show.
#[must_use]
pub fn tiles(view: &View<'_>) -> Vec<Tile> {
    view.cards
        .iter()
        .filter_map(|row| {
            let Hit::Row(at) = ids::decode(row.id) else {
                return None;
            };
            let face = row.card.as_ref()?;
            Some(Tile {
                key: KEYS
                    .get(at)
                    .map_or_else(|| (at + 1).to_string(), |k| (*k).to_owned()),
                cost: face.cost.clone(),
                name: face.name.clone(),
                kind: face.kind.clone(),
                rarity: face.rarity.clone(),
                text: text::spans(&face.text),
                target: Target::Card(CardId(face.card)),
                dim: matches!(row.state, ButtonState::Dim(_)),
                selected: row.state == ButtonState::Selected,
                glow: face.glow,
            })
        })
        .collect()
}

/// What a card choice of the fight asks, for the hand frame's title row: `Survivor · Choose a
/// card to discard (0 of 1)`.
#[must_use]
pub fn decision_title(view: &View<'_>) -> Option<String> {
    view.choice.map(|choice| choice.title.text())
}
