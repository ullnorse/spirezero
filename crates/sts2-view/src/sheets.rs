//! The run menu and the full-screen lists behind it, each built as a [`Screen`] whose `sheet`
//! holds the list.
//!
//! The lists are the deck, the relics, the potions and powers, the piles of a fight, the
//! fight's log, the rooms visited and the enlarged map.
//!
//! A list is a [`DetailSheet`] of [`Rich`] lines (the map's grid is the screen's one
//! [`Block::Map`], its legend the sheet's line); the `≡` menu is a [`MenuSheet`] with a row per
//! entry. A client scrolls the lines itself; which sheet is open, and how far it is scrolled,
//! is [`crate::ui::Ui`]'s.

use sts2_core::combat::{CombatState, CreatureId};
use sts2_core::content::cards::CardKind;
use sts2_core::run::Run;

use crate::describe::Describer;
use crate::model::{
    Block, Button, ButtonId, ButtonState, CardFace, DetailSheet, Key, MenuSheet, Screen, Sheet,
};
use crate::rooms::{self, Visit};
use crate::text::{self, Rich, RichSpan, Role};
use crate::{menu, names};

mod log;

/// The legend under the enlarged map.
const LEGEND: &str = "M monster  E elite  ? unknown  $ shop  T treasure  R rest  B boss  ● you";

/// Which sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The `≡` run menu.
    Menu,
    Deck,
    Relics,
    /// The potions and, in a fight, the orbs, blades and every creature with its powers.
    Info,
    /// The act's map, enlarged.
    Map,
    /// The rooms visited.
    History,
    /// The fight's log, scrolled from its newest line.
    Log,
    /// The draw pile, sorted: the game hides its order.
    DrawPile,
    DiscardPile,
    ExhaustPile,
}

impl Kind {
    /// The sheet exists only in a fight (the piles, the log).
    #[must_use]
    pub const fn fight_only(self) -> bool {
        matches!(
            self,
            Self::Log | Self::DrawPile | Self::DiscardPile | Self::ExhaustPile
        )
    }

    /// The sheet is read from its newest (last) line.
    #[must_use]
    pub const fn from_bottom(self) -> bool {
        matches!(self, Self::Log)
    }
}

/// What a row of the `≡` menu does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Entry {
    /// Opens a sheet.
    Sheet(Kind),
    /// The client's help, settings or quit.
    Help,
    Settings,
    Quit,
    /// Throws away the potion in this belt slot (asks first).
    Toss(usize),
}

/// One entry of the menu: the key that does it from any screen, its label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    pub key: char,
    pub label: &'static str,
    pub entry: Entry,
}

/// The menu's entries; `fight` adds the fight's log and piles.
#[must_use]
pub fn menu_entries(fight: bool) -> Vec<MenuEntry> {
    let e = |key, label, entry| MenuEntry { key, label, entry };
    let mut entries = vec![
        e('d', "Deck", Entry::Sheet(Kind::Deck)),
        e('v', "Relics", Entry::Sheet(Kind::Relics)),
        e('i', "Potions and powers", Entry::Sheet(Kind::Info)),
        e('m', "Map", Entry::Sheet(Kind::Map)),
    ];
    if fight {
        entries.extend([
            e('l', "Log", Entry::Sheet(Kind::Log)),
            e('a', "Draw pile", Entry::Sheet(Kind::DrawPile)),
            e('s', "Discard pile", Entry::Sheet(Kind::DiscardPile)),
            e('x', "Exhaust pile", Entry::Sheet(Kind::ExhaustPile)),
        ]);
    }
    entries.extend([
        e('h', "Rooms visited", Entry::Sheet(Kind::History)),
        e('?', "Help", Entry::Help),
        e('o', "Settings", Entry::Settings),
        e('q', "Quit (the run is saved)", Entry::Quit),
    ]);
    entries
}

/// What the builders read.
#[derive(Debug)]
pub struct Context<'a> {
    pub run: &'a Run,
    pub describer: &'a Describer,
    /// The rooms entered so far, oldest first.
    pub visits: &'a [Visit],
    /// The belt slot whose throw-away row is asking, with the row's question.
    pub tossing: Option<(usize, String)>,
}

/// One row of the menu with what it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuRow {
    pub entry: Entry,
    /// Its button: the id is the row's place (1-based).
    pub button: Button,
}

/// The menu's rows: each entry, and under Potions a row per potion that throws it away (its
/// belt slot's number), with the potion's text.
#[must_use]
pub fn menu_rows(ctx: &Context<'_>) -> Vec<MenuRow> {
    let fight = ctx.run.combat().is_some();
    let mut rows: Vec<(Entry, Key, Rich, Vec<Rich>, bool)> = Vec::new();
    for e in menu_entries(fight) {
        rows.push((e.entry, Key::Char(e.key), plain(e.label), Vec::new(), true));
        if e.entry != Entry::Sheet(Kind::Info) {
            continue;
        }
        let discardable = ctx.run.discard_actions();
        for (slot, id) in ctx.run.potions().iter().enumerate() {
            let Some(id) = id else {
                continue;
            };
            let label = match &ctx.tossing {
                Some((s, question)) if *s == slot => question.clone(),
                _ => format!("Throw away {}", names::potion(id)),
            };
            let key = u8::try_from(slot + 1).map_or(Key::Enter, Key::Digit);
            rows.push((
                Entry::Toss(slot),
                key,
                plain(label),
                ctx.describer
                    .potion(id)
                    .map_or_else(Vec::new, |d| text::lines(&d, Role::Plain)),
                discardable.contains(&sts2_core::run::RunAction::DiscardPotion(slot)),
            ));
        }
    }
    rows.into_iter()
        .enumerate()
        .map(|(at, (entry, key, label, detail, enabled))| MenuRow {
            entry,
            button: Button {
                key: Some(key),
                detail,
                state: if enabled {
                    ButtonState::Enabled
                } else {
                    ButtonState::Dim("it cannot be thrown away now".to_owned())
                },
                ..Button::row(u32::try_from(at + 1).unwrap_or(u32::MAX), label)
            },
        })
        .collect()
}

/// The sheet `kind` as a screen; `None` for a fight-only sheet outside a fight.
#[must_use]
pub fn screen(kind: Kind, ctx: &Context<'_>) -> Option<Screen> {
    let cards = cards(kind, ctx);
    let (title, lines) = match kind {
        Kind::Menu => {
            let title = plain("Menu");
            let rows = menu_rows(ctx).into_iter().map(|r| r.button).collect();
            return Some(Screen {
                title: Some(title.clone()),
                sheet: Some(Sheet::Menu(MenuSheet {
                    title,
                    rows,
                    page: None,
                })),
                ..Screen::default()
            });
        }
        Kind::Deck => deck(ctx),
        Kind::Relics => relics(ctx),
        Kind::Info => info(ctx),
        Kind::History => history(ctx),
        Kind::Map => return Some(full_map(ctx)),
        Kind::Log => {
            let combat = ctx.run.combat()?;
            (
                format!("Log · Round {}", combat.state().round),
                log::lines(combat),
            )
        }
        Kind::DrawPile | Kind::DiscardPile | Kind::ExhaustPile => pile(kind, ctx)?,
    };
    let mut screen = detail(title, lines, Vec::new());
    if let Some(Sheet::Detail(sheet)) = screen.sheet.as_mut() {
        sheet.cards = cards;
    }
    Some(screen)
}

/// The deck's or a pile's cards as faces, in the list's order (the draw pile sorted by name,
/// its order hidden); none for another sheet.
fn cards(kind: Kind, ctx: &Context<'_>) -> Vec<CardFace> {
    if kind == Kind::Deck {
        return ctx
            .run
            .state()
            .deck
            .iter()
            .map(|&c| rooms::deck_face(ctx.describer, c))
            .collect();
    }
    let Some(combat) = ctx.run.combat() else {
        return Vec::new();
    };
    let p = &combat.state().player;
    let ids = match kind {
        Kind::DrawPile => &p.draw,
        Kind::DiscardPile => &p.discard,
        Kind::ExhaustPile => &p.exhaust,
        _ => return Vec::new(),
    };
    let mut faces: Vec<CardFace> = ids
        .iter()
        .map(|&id| crate::combat::card_face(combat, ctx.describer, id))
        .collect();
    if kind == Kind::DrawPile {
        faces.sort_by(|a, b| (&a.name, a.card).cmp(&(&b.name, b.card)));
    }
    faces
}

fn detail(title: String, lines: Vec<Rich>, blocks: Vec<Block>) -> Screen {
    let title = plain(title);
    Screen {
        title: Some(title.clone()),
        blocks,
        sheet: Some(Sheet::Detail(DetailSheet {
            title,
            lines,
            buttons: Vec::new(),
            cards: Vec::new(),
        })),
        ..Screen::default()
    }
}

fn span(role: Role, bold: bool, text: impl Into<String>) -> RichSpan {
    RichSpan {
        fx: None,
        role,
        bold,
        text: text.into(),
    }
}

fn plain(text: impl Into<String>) -> Rich {
    Rich(vec![span(Role::Plain, false, text)])
}

fn bold(text: impl Into<String>) -> Rich {
    Rich(vec![span(Role::Plain, true, text)])
}

/// The lines of `markup`, indented under a heading.
fn indented(markup: &str) -> Vec<Rich> {
    text::lines(markup, Role::Body)
        .into_iter()
        .map(|mut line| {
            line.0.insert(0, span(Role::Plain, false, "   "));
            line
        })
        .collect()
}

/// A heading line with its count (`3x Strike`) and the text under it.
fn counted(count: usize, label: &str, text: Option<&str>, lines: &mut Vec<Rich>) {
    let mut line = Rich::default();
    if count > 1 {
        line.0.push(span(Role::Gold, false, format!("{count}x ")));
    }
    line.0.push(span(Role::Plain, false, label));
    lines.push(line);
    if let Some(text) = text {
        lines.extend(indented(text));
    }
}

fn deck(ctx: &Context<'_>) -> (String, Vec<Rich>) {
    let deck = &ctx.run.state().deck;
    // Identical cards share a line: `5x Strike`.
    let mut groups: Vec<(String, usize, Option<String>)> = Vec::new();
    for c in deck {
        let label = names::run_card_detail(*c);
        match groups.iter_mut().find(|(l, ..)| *l == label) {
            Some((_, n, _)) => *n += 1,
            None => groups.push((label, 1, ctx.describer.run_card(*c))),
        }
    }
    let mut lines = Vec::new();
    for (label, n, text) in &groups {
        counted(*n, label, text.as_deref(), &mut lines);
    }
    (format!("Deck ({} cards)", deck.len()), lines)
}

/// The rooms entered so far, with the HP and gold you left them with.
fn history(ctx: &Context<'_>) -> (String, Vec<Rich>) {
    let mut lines = Vec::new();
    let mut act = 0;
    for v in ctx.visits {
        if v.act != act {
            act = v.act;
            lines.push(bold(format!("Act {act}")));
        }
        let hp = if v.hp_in == v.hp_out {
            format!("♥ {}/{}", v.hp_out, v.max_hp)
        } else {
            format!("♥ {} → {}/{}", v.hp_in, v.hp_out, v.max_hp)
        };
        lines.push(Rich(vec![
            span(Role::Gold, false, format!("Floor {}", v.floor)),
            span(Role::Body, false, format!(" · {hp} · ◉ {}", v.gold_out)),
        ]));
        lines.push(plain(format!("   {}", v.what)));
    }
    (format!("History ({} rooms)", ctx.visits.len()), lines)
}

/// Identical cards of a pile: same label and text.
struct Group {
    /// Sorted by name, then upgrade level.
    order: (&'static str, u8),
    label: String,
    text: Option<String>,
    count: usize,
}

/// A pile of the fight as counts: identical cards share a line (`3x Strike`), sorted by name.
/// The draw pile's order is hidden in the game, so it is never shown; the others read alike.
fn pile(kind: Kind, ctx: &Context<'_>) -> Option<(String, Vec<Rich>)> {
    let state = ctx.run.combat()?.state();
    let (name, ids) = match kind {
        Kind::DrawPile => ("Draw pile", &state.player.draw),
        Kind::DiscardPile => ("Discard pile", &state.player.discard),
        _ => ("Exhaust pile", &state.player.exhaust),
    };
    let mut groups: Vec<Group> = Vec::new();
    for &id in ids {
        let card = state.card(id);
        let tag = names::card_tag(state, id).map_or_else(String::new, |t| format!(" {t}"));
        let label = format!(
            "{}{tag} ({:?}, {} energy)",
            names::card(state, id),
            card.card_type(),
            state.energy_cost(id).max(0)
        );
        let text = ctx.describer.hand_card(state, id);
        match groups
            .iter_mut()
            .find(|g| g.label == label && g.text == text)
        {
            Some(group) => group.count += 1,
            None => groups.push(Group {
                order: (card.kind.name(), card.upgrade_level),
                label,
                text,
                count: 1,
            }),
        }
    }
    groups.sort_by(|a, b| (a.order, &a.label).cmp(&(b.order, &b.label)));
    let mut lines = Vec::new();
    for g in &groups {
        counted(g.count, &g.label, g.text.as_deref(), &mut lines);
    }
    if groups.is_empty() {
        lines.push(plain("(empty)"));
    }
    let hidden = if kind == Kind::DrawPile {
        ", the order is hidden"
    } else {
        ""
    };
    Some((format!("{name} ({}){hidden}", ids.len()), lines))
}

fn relics(ctx: &Context<'_>) -> (String, Vec<Rich>) {
    let mut lines = Vec::new();
    let run = ctx.run;
    let combat = run.combat().map(sts2_core::Combat::state);
    // In a fight the relics' fields are the fight's.
    let in_fight = combat
        .map(|c| &c.player.relics)
        .filter(|r| r.len() == run.state().relics.len());
    for (i, r) in run.state().relics.iter().enumerate() {
        let data = in_fight.map_or(r.data, |fight| fight[i].data);
        let counter = r
            .kind
            .counter(&data, combat)
            .map_or_else(String::new, |n| format!(" ({n})"));
        lines.push(bold(format!("◆ {}{counter}", r.kind.name())));
        if let Some(text) = ctx.describer.owned_relic(r.kind, &data) {
            lines.extend(indented(&text));
        }
    }
    ("Relics".to_owned(), lines)
}

/// The potions on the belt and, in a fight, the orbs, the Sovereign Blades and every creature
/// (HP, Block, powers), with their text.
fn info(ctx: &Context<'_>) -> (String, Vec<Rich>) {
    let mut lines = vec![bold("Potions")];
    let potions = ctx.run.potions();
    if potions.iter().all(Option::is_none) {
        lines.push(Rich(vec![span(Role::Muted, false, "No potions")]));
    }
    for (i, potion) in potions.iter().enumerate() {
        let Some(id) = potion else {
            continue;
        };
        lines.push(plain(format!("{}. {}", i + 1, names::potion(id))));
        if let Some(text) = ctx.describer.potion(id) {
            lines.extend(indented(&text));
        }
    }
    if let Some(combat) = ctx.run.combat() {
        let state = combat.state();
        lines.extend(orb_lines(ctx.describer, state));
        lines.extend(blade_lines(ctx.describer, state));
        let creatures = std::iter::once(state.player.creature)
            .chain(state.osty())
            .chain(state.enemies.iter().copied());
        for id in creatures {
            lines.push(plain(""));
            lines.extend(creature_lines(ctx.describer, state, id));
        }
    }
    ("Info".to_owned(), lines)
}

/// The orb slots, the next to evoke first, with each orb's text (its values now).
fn orb_lines(describer: &Describer, state: &CombatState) -> Vec<Rich> {
    let p = &state.player;
    if p.orb_slots == 0 && p.orbs.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![plain(""), bold("Orbs (the first is evoked next)")];
    let slots = usize::from(p.orb_slots).max(p.orbs.len());
    for slot in 0..slots {
        if let Some(&orb) = p.orbs.get(slot) {
            let passive = state.orb_passive_val(orb).to_int_truncating();
            let evoke = state.orb_evoke_val(orb).to_int_truncating();
            lines.push(plain(format!(
                "{}. {} {passive}/{evoke}",
                slot + 1,
                describer.orb_title(orb.kind)
            )));
            if let Some(text) = describer.orb(state, orb) {
                lines.extend(indented(&text));
            }
        } else {
            let (title, text) = describer.empty_orb_slot().unwrap_or(("Empty slot", ""));
            lines.push(plain(format!("{}. {title}", slot + 1)));
            if !text.is_empty() {
                lines.extend(indented(text));
            }
        }
    }
    lines
}

/// The Sovereign Blades, where each is and its text (its damage now).
fn blade_lines(describer: &Describer, state: &CombatState) -> Vec<Rich> {
    let p = &state.player;
    let piles = [
        ("hand", &p.hand),
        ("draw pile", &p.draw),
        ("discard pile", &p.discard),
        ("exhaust pile", &p.exhaust),
        ("play", &p.play),
    ];
    let mut lines = Vec::new();
    for (place, ids) in piles {
        for &id in ids {
            if state.card(id).kind != CardKind::SovereignBlade {
                continue;
            }
            if lines.is_empty() {
                lines.push(plain(""));
                lines.push(bold("Sovereign Blade"));
            }
            lines.push(plain(format!("{} ({place})", names::card(state, id))));
            if let Some(text) = describer.hand_card(state, id) {
                lines.extend(indented(&text));
            }
        }
    }
    lines
}

/// A creature's HP, Block and powers with their text.
fn creature_lines(describer: &Describer, state: &CombatState, id: CreatureId) -> Vec<Rich> {
    let name = names::creature(state, id);
    let c = state.creature(id);
    let status = if c.is_alive() {
        let block = if c.block > 0 {
            format!(", Block {}", c.block)
        } else {
            String::new()
        };
        format!("HP {}/{}{block}", c.hp, c.max_hp)
    } else {
        "dead".to_owned()
    };
    let mut lines = vec![Rich(vec![
        span(Role::Plain, true, name.clone()),
        span(Role::Plain, false, format!("  {status}")),
    ])];
    for p in c.powers.iter().filter(|p| p.kind.is_visible()) {
        lines.push(plain(format!("{} {}", names::power(p.kind), p.amount)));
        if let Some(text) = describer.power(
            p.kind,
            p.amount,
            state.power_data(p.id),
            &name,
            id == state.player.creature,
        ) {
            lines.extend(indented(&text));
        }
    }
    lines
}

/// The act's map, enlarged: the grid is the screen's block, the legend the sheet's line.
fn full_map(ctx: &Context<'_>) -> Screen {
    let state = ctx.run.state();
    let items = menu::items(ctx.run, ctx.describer, None);
    let grid = rooms::map_grid(ctx.run, &items);
    detail(
        format!(
            "Act {} map: {}",
            state.act_index + 1,
            names::act(state.act().act.id)
        ),
        vec![Rich(vec![span(Role::Body, false, LEGEND)])],
        vec![Block::Map(grid)],
    )
}

/// The ids the menu's rows are numbered by: a row's place, 1-based.
#[must_use]
pub fn entry_of(ctx: &Context<'_>, id: ButtonId) -> Option<Entry> {
    let at = usize::try_from(id.0).ok()?.checked_sub(1)?;
    menu_rows(ctx).get(at).map(|r| r.entry)
}

#[cfg(test)]
mod tests;
