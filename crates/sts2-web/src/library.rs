//! The compendium's lists (card library, relic collection and potion lab):
//! every card, relic and potion the engine ports, with the game's text. Needs no run.

use std::fmt::Write;
use std::rc::Rc;

use sts2_core::content::cards::{CardKind, Keywords};
use sts2_core::content::potions::PotionKind;
use sts2_core::content::relics::RelicKind;
use sts2_core::run::RunCard;
use sts2_view::describe::Describer;
use sts2_view::rooms;
use sts2_view::text::Role;
use sts2_view::{names, text};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::TEXT;

/// How many of each the compendium shows, as JSON `[cards, relics, potions]`.
#[wasm_bindgen]
#[must_use]
pub fn library_counts() -> String {
    serde_json::json!([
        CardKind::ALL.len(),
        RelicKind::ALL.len(),
        PotionKind::ALL.len()
    ])
    .to_string()
}

/// `count` of the compendium's `kind` (`cards`, `relics` or `potions`) from `from`, as JSON.
///
/// The page fetches them a few at a time. A card is `{now, up}`: its face (`CardFace`) as it is
/// and, when it can be upgraded, upgraded (else `null`); a relic or potion is `{id, name, text}`
/// with `text` as rich lines. Relic and potion groups (rarity, pool) are the page's own data.
#[wasm_bindgen]
#[must_use]
pub fn library_part(kind: &str, from: usize, count: usize) -> String {
    let d = Describer::new(TEXT.with_borrow(Rc::clone));
    let lines =
        |markup: Option<String>| markup.map_or_else(Vec::new, |m| text::lines(&m, Role::Plain));
    let out: Vec<serde_json::Value> = match kind {
        "cards" => CardKind::ALL
            .iter()
            .skip(from)
            .take(count)
            .map(|kind| {
                let now = RunCard::new(kind.entry());
                let up = now.is_upgradable().then(|| {
                    let mut up = now;
                    up.upgrade();
                    rooms::card_face(&d, up)
                });
                serde_json::json!({ "now": rooms::card_face(&d, now), "up": up })
            })
            .collect(),
        "relics" => RelicKind::ALL
            .iter()
            .skip(from)
            .take(count)
            .map(|kind| {
                serde_json::json!({
                    "id": kind.id(),
                    "name": names::relic(kind.id()),
                    "text": lines(d.relic(kind.id())),
                })
            })
            .collect(),
        "potions" => PotionKind::ALL
            .iter()
            .skip(from)
            .take(count)
            .map(|kind| {
                serde_json::json!({
                    "id": kind.id(),
                    "name": names::potion(kind.id()),
                    "text": lines(d.potion(kind.id())),
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    serde_json::Value::Array(out).to_string()
}

/// One relic's or potion's `{id, name, text}` by `kind` (`relics`, `potions`) and id, as JSON (`null`
/// when unknown): a reward's or shop item's hover tip (GD28).
#[wasm_bindgen]
#[must_use]
pub fn library_item(kind: &str, id: &str) -> String {
    let d = Describer::new(TEXT.with_borrow(Rc::clone));
    let lines =
        |markup: Option<String>| markup.map_or_else(Vec::new, |m| text::lines(&m, Role::Plain));
    let item = match kind {
        "relics" => RelicKind::ALL.iter().find(|k| k.id() == id).map(|k| {
            serde_json::json!({ "id": k.id(), "name": names::relic(k.id()), "text": lines(d.relic(k.id())) })
        }),
        "potions" => PotionKind::ALL.iter().find(|k| k.id() == id).map(|k| {
            serde_json::json!({ "id": k.id(), "name": names::potion(k.id()), "text": lines(d.potion(k.id())) })
        }),
        _ => None,
    };
    item.map_or_else(|| "null".to_owned(), |v| v.to_string())
}

/// One card's face by its id (`SHIV`), as JSON, upgraded when `upgraded` and it can be; `null` for
/// an id the engine does not port. The hover tips' card previews use it (GD28).
#[wasm_bindgen]
#[must_use]
pub fn library_card(id: &str, upgraded: bool) -> String {
    let d = Describer::new(TEXT.with_borrow(Rc::clone));
    let Some(kind) = CardKind::ALL.iter().find(|k| k.entry() == id) else {
        return "null".to_owned();
    };
    let mut card = RunCard::new(kind.entry());
    if upgraded && card.is_upgradable() {
        card.upgrade();
    }
    serde_json::to_string(&rooms::card_face(&d, card)).unwrap_or_default()
}

/// [`Web::glossary`](crate::Web::glossary) without a run, for the compendium's tips: the game's
/// glossary for the lit words of a text (`["Doom","Block"]` as JSON), as `[{title, text}]` JSON.
#[wasm_bindgen]
#[must_use]
pub fn library_glossary(words: &str) -> String {
    let d = Describer::new(TEXT.with_borrow(Rc::clone));
    let words: Vec<String> = serde_json::from_str(words).unwrap_or_default();
    let markup = words.iter().fold(String::new(), |mut m, w| {
        let _ = write!(m, "[gold]{w}[/gold] ");
        m
    });
    let terms: Vec<_> = d
        .glossary(&markup, Keywords::NONE)
        .into_iter()
        .map(|t| serde_json::json!({ "title": t.title, "text": text::inline(&t.text, Role::Body) }))
        .collect();
    serde_json::Value::Array(terms).to_string()
}
