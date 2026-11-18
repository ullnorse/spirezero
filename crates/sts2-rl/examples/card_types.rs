//! Prints `name<TAB>type` for every card in `vocab/cards.txt` (RLN19's deck composition).
use std::io::{self, Write};

use sts2_core::run::RunCard;

fn main() {
    let text = include_str!("../vocab/cards.txt");
    let mut out = io::stdout().lock();
    for name in text.lines() {
        let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
        let t = RunCard::new(leaked)
            .card_type()
            .map_or_else(|| "?".to_owned(), |t| format!("{t:?}"));
        writeln!(out, "{name}\t{t}").expect("stdout");
    }
}
