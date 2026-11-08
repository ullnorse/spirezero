//! The fight, in the column under the top bar: the enemies stacked (number and name, intent,
//! HP bar with Block, powers), the player (HP and Block, what the enemy turn takes in one
//! line, what the last turn or play did in one more line, powers, the character's
//! resources), the hand as a framed list of rows, the energy line, and the End Turn button.
//! On a big screen ([`scale::big`]) the fight draws larger first ([`Big`]): the HP bars two
//! rows thick, each card two rows (its name, then its text). Short of rows the fight gives those up
//! first, biggest first, then drops the blank rows, the last-turn line, shrinks the button,
//! puts the enemies two to a row, drops the hand's frame and the powers, then puts each enemy
//! on one row ([`Plan`]); past that the hand scrolls to the focused card and End Turn stays.
//!
//! Every number shown comes from sts2-core: intents as `CombatState::intents` reports them,
//! what the enemy turn does to the player from `Combat::incoming`, card text with the card's
//! live vars, outcomes from `Combat::play_outcome_until_draw`. The screen does no game
//! arithmetic of its own.
//!
//! Everything the mouse can click is registered as it is drawn: the creatures (targets), the
//! cards, the potions, the End Turn button, the piles. The keyboard focus (`▸`, reverse video)
//! rests on a card, a target, a potion or End Turn; while it rests on a card the enemies show
//! what that card would do to them, drawn into their HP bars so that nothing on the screen
//! moves as the focus moves: every line keeps its place whether or not it shows a preview.
//! The focused card's row opens to its text in full; the hand's list has room for the longest
//! kept while the screen has the rows to spare, and scrolls to the focused card otherwise, so
//! the focus moving opens and closes text inside the list and moves nothing else.

mod enemies;
mod hand;
mod player;
pub mod tiles;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sts2_core::combat::{CreatureId, Event, Side};
use sts2_view::model::{
    Block, Button as Row, ButtonState, ChoiceSheet, EnemyRow, IncomingRow, OrbRow, PetRow,
    PowerRow, Preview as Outcome, Screen, Sheet, YouRow,
};
use sts2_view::text::Rich;

use crate::app::{App, Overlay};
use crate::hit::{Hits, Target, UiAction};
use crate::ui::button::{self, Button};
use crate::ui::column::{self, Stack};
use crate::ui::flow::{self, Flow};
use crate::ui::motion;
use crate::ui::scale;
use crate::ui::theme::{self, Role, style};

/// What the fight screen reads: the model's [`Screen`] cut into its parts, once per frame.
pub struct View<'a> {
    pub app: &'a App,
    pub enemies: Vec<&'a EnemyRow>,
    pub you: &'a YouRow,
    pub pets: Vec<&'a PetRow>,
    pub orbs: Vec<&'a OrbRow>,
    /// What the enemies' next turn does to the player; `None` outside the play phase. When it
    /// is `stale` it is what the turn's last play began with, kept for a card choice of the
    /// fight (the engine has no preview while one is open): drawn dim.
    pub incoming: Option<&'a IncomingRow>,
    pub last: Option<&'a Rich>,
    /// The hand's rows, or a card choice's.
    pub cards: &'a [Row],
    /// The fight's card choice, if one is open.
    pub choice: Option<&'a ChoiceSheet>,
    /// End Turn, or a card choice's Done.
    pub main: Option<&'a Row>,
    /// What a card or potion waiting for a target, or a dragged card, says.
    pub title: Option<&'a Rich>,
}

impl<'a> View<'a> {
    /// `None` when the screen has no player row (it is not a fight).
    fn new(app: &'a App, screen: &'a Screen) -> Option<Self> {
        let mut you = None;
        let (mut enemies, mut pets, mut orbs) = (Vec::new(), Vec::new(), Vec::new());
        let (mut incoming, mut last, mut cards) = (None, None, &[][..]);
        for block in &screen.blocks {
            match block {
                Block::Enemy(e) => enemies.push(e.as_ref()),
                Block::You(y) => you = Some(y),
                Block::Pet(p) => pets.push(p.as_ref()),
                Block::Orbs(o) => orbs.push(o),
                Block::Incoming(i) => incoming = Some(i),
                Block::Last(l) => last = Some(l),
                Block::List { rows, .. } => cards = rows.as_slice(),
                _ => {}
            }
        }
        let choice = match &screen.sheet {
            Some(Sheet::Choice(choice)) => Some(choice),
            _ => None,
        };
        Some(Self {
            app,
            enemies,
            you: you?,
            pets,
            orbs,
            incoming,
            last,
            cards: choice.map_or(cards, |c| c.rows.as_slice()),
            choice,
            main: screen.primary.as_ref(),
            title: screen.title.as_ref(),
        })
    }

    /// Whether the player can act: their turn, no card choice pending.
    pub fn in_turn(&self) -> bool {
        self.choice.is_none() && self.main.is_some_and(|m| m.state != ButtonState::Disabled)
    }
}

/// How the fight is laid out in the rows there are: how much it has given up, from nothing
/// (0) to everything it can (7). Each step gives up one more thing: the blank rows, the
/// last-turn line's row, the End Turn button's frame, an enemy a row (two to a row instead),
/// the hand's frame, the powers (they stay a tooltip and `i` away), the enemies' rows (one
/// row each: name, intent and HP). Past that the hand scrolls to the focused card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan(u8);

impl Plan {
    const ROOMIEST: Self = Self(0);
    const TIGHTEST: Self = Self(7);

    /// Blank rows before the enemies, between them and after them.
    const fn gaps(self) -> bool {
        self.0 < 1
    }

    /// The row between the player and the hand, with what the last turn or play did.
    const fn last(self) -> bool {
        self.0 < 2
    }

    /// The End Turn button framed (three rows) rather than one row.
    const fn big_button(self) -> bool {
        self.0 < 3
    }

    /// Two enemies to a row.
    const fn pairs(self) -> bool {
        self.0 >= 4 && !self.compact()
    }

    /// Each enemy on one row: `1 Leaf Slime (S)  ⚔ 3  ████░░ 8/14`.
    pub const fn compact(self) -> bool {
        self.0 >= 7
    }

    /// The hand in a frame.
    const fn framed(self) -> bool {
        self.0 < 5
    }

    /// The creatures' powers.
    const fn powers(self) -> bool {
        self.0 < 6
    }
}

/// The most blank rows before the enemies and between them and the player on a tall screen.
const MAX_GAP: u16 = 2;

/// What a big screen draws large ([`scale::big`]); all off elsewhere, and given up one by one
/// when the rows run short, the card rows first and the thick bars last.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Big {
    /// Every HP bar two rows thick.
    pub thick: bool,
    /// Each card row two rows: the name, then its text.
    pub tall_cards: bool,
    /// A blank row under each card row too (three rows a card), with rows to spare.
    pub pad_cards: bool,
}

impl Big {
    const NONE: Self = Self {
        thick: false,
        tall_cards: false,
        pad_cards: false,
    };
    const ALL: Self = Self {
        thick: true,
        tall_cards: true,
        pad_cards: true,
    };
}

/// The layouts tried, roomiest first: on a big screen everything large, then the card rows'
/// padding, then their second row, then the blank rows ([`Plan`] 1), then the thick bars;
/// from there the plans give up the rest.
fn candidates() -> Vec<(Plan, Big)> {
    let mut out = Vec::new();
    if scale::big() {
        let tall = Big {
            pad_cards: false,
            ..Big::ALL
        };
        let one_row = Big {
            tall_cards: false,
            ..tall
        };
        let thin = Big {
            thick: false,
            ..one_row
        };
        out.extend([
            (Plan::ROOMIEST, Big::ALL),
            (Plan::ROOMIEST, tall),
            (Plan::ROOMIEST, one_row),
            (Plan(1), one_row),
            (Plan(1), thin),
        ]);
    }
    out.extend((Plan::ROOMIEST.0..=Plan::TIGHTEST.0).map(|p| (Plan(p), Big::NONE)));
    out
}

/// The parts of the fight screen, built once per frame.
struct Parts {
    hand: Vec<Button>,
    /// The hand as card tiles instead of the list, on a big screen ([`tiles`]).
    tiles: Option<Vec<tiles::Tile>>,
    /// The main button: End Turn, or a card choice's Done.
    main: Option<hand::Main>,
    /// The rows of the line between the hand and the button.
    message: u16,
}

impl Parts {
    /// The rows the fight needs `width` wide in `plan`.
    fn height(&self, view: &View<'_>, plan: Plan, big: Big, width: u16) -> u16 {
        let gap = u16::from(plan.gaps());
        let main = match (&self.main, plan.big_button()) {
            (None, _) => 0,
            (Some(_), true) => scale::primary_rows(),
            (Some(_), false) => 1,
        };
        gap + enemies::height(view, plan, big, width, gap)
            + gap
            + player::height(view, plan, big, width)
            + u16::from(plan.last())
            + self.closed(plan, width)
            + self.message
            + main
    }

    /// The rows the hand takes with no text open: the list's, or the tiles'.
    fn closed(&self, plan: Plan, width: u16) -> u16 {
        self.tiles.as_ref().map_or_else(
            || button::closed_height(&self.hand, width, plan.framed()),
            |t| tiles::height(t.len(), width),
        )
    }

    /// The rows the hand would add if it held [`SIZING_HAND`] cards: the screen is laid out for
    /// at least that many, so playing cards leaves the creatures and End Turn where they are.
    fn pad_rows(&self, plan: Plan, big: Big, width: u16) -> u16 {
        let n = self.hand.len();
        if n >= SIZING_HAND {
            return 0;
        }
        if self.tiles.is_some() {
            return tiles::height(SIZING_HAND, width).saturating_sub(tiles::height(n, width));
        }
        let padded = if big.tall_cards {
            big.pad_cards
        } else {
            scale::padded()
        };
        let per = if n == 0 {
            1 + u16::from(big.tall_cards) + u16::from(padded)
        } else {
            let frame = 2 * u16::from(plan.framed());
            let rows = button::closed_height(&self.hand, width, plan.framed()) - frame;
            rows.div_ceil(flow::coord(n))
        };
        per * flow::coord(SIZING_HAND - n)
    }

    /// The rows the hand's list gets in `plan` with `spare` rows to give: its rows with no
    /// text open, and the rows the longest text in full needs while they are spare (short of
    /// them the list scrolls to the focused card).
    fn hand_rows(&self, plan: Plan, width: u16, spare: u16) -> u16 {
        if self.tiles.is_some() {
            return self.closed(plan, width);
        }
        let closed = button::closed_height(&self.hand, width, plan.framed());
        let open = button::list_height(&self.hand, width, plan.framed());
        closed + (open - closed).min(spare)
    }
}

/// The fewest columns the hand is drawn as card tiles in.
const TILE_COLUMNS: u16 = 110;

/// The fewest cards the fight screen is laid out for (a turn's usual draw).
const SIZING_HAND: usize = 5;

/// Draws the fight in `area`, the rows under the top bar.
pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App, hits: &mut Hits) {
    if area.is_empty() {
        return;
    }
    let Some(screen) = app.fight_screen() else {
        return;
    };
    draw_screen(frame, area, app, &screen, hits);
}

/// Draws the fight `screen` describes in `area`; `app` supplies the notice and the keys.
pub fn draw_screen(frame: &mut Frame<'_>, area: Rect, app: &App, screen: &Screen, hits: &mut Hits) {
    if area.is_empty() {
        return;
    }
    let Some(view) = View::new(app, screen) else {
        return;
    };
    observe(&view);
    let Some((plan, big, parts)) = layout(&view, area) else {
        return;
    };
    let Rows {
        hand: hand_rows,
        hand_area,
        hints: hint_rows,
        main: main_rows,
        blanks: (before, between, after),
    } = rows(&view, &parts, plan, big, area);
    let bottom = parts.message + main_rows + hint_rows;
    let top = Rect {
        height: area.height.saturating_sub(hand_area + bottom),
        ..area
    };
    let mut stack = Stack::new(top);
    stack.skip(before);
    // The battlefield first, so the creatures drawn on it are found before it.
    let scene = enemies::height(&view, plan, big, area.width, between)
        + after
        + player::height(&view, plan, big, area.width);
    let scene = Rect {
        height: scene.min(stack.left()),
        ..stack.rest()
    };
    hits.push(scene, Target::Battlefield);
    enemies::draw(frame, &mut stack, &view, plan, big, between, hits);
    // The blank rows between the enemies and you hold the banner, when there are any.
    let gap = stack.take(after);
    player::draw(frame, &mut stack, &view, plan, big, hits);
    draw_banner(frame, gap);
    if plan.last() {
        draw_last(frame, stack.take(1), &view);
    }
    let used = top.height - stack.left();
    let mut stack = Stack::new(area);
    stack.skip(used);
    let list = stack.take(hand_rows);
    // The line, the main button and the hints sit on the screen's last rows.
    let mut stack = Stack::new(area);
    stack.skip(area.height.saturating_sub(bottom));
    // A card choice's prompt is the hand frame's title row; without the frame it is the line
    // under the hand.
    let title = (plan.framed() && view.choice.is_some())
        .then(|| hand::decision_title(&view))
        .flatten();
    if let Some(tiles) = &parts.tiles {
        tiles::draw(frame, list, tiles, hits);
    } else {
        tiles::drew_list();
        button::draw_list_titled(
            frame,
            list,
            &parts.hand,
            plan.framed(),
            title.as_deref(),
            0,
            hits,
        );
    }
    draw_message(
        frame,
        stack.take(parts.message),
        &view,
        title.is_some(),
        hits,
    );
    if let Some(main) = parts.main {
        button::draw_primary(
            frame,
            stack.take(main_rows),
            main.key,
            main.label,
            main.action,
            main.enabled,
            hits,
        );
    }
    let potions = app.potion_keys();
    let potions = if potions.is_empty() {
        String::new()
    } else {
        format!("{potions} potion · ")
    };
    crate::ui::hints(
        frame,
        &stack,
        &format!(
            "{} card · Enter play · 1-9 play · e End Turn · {potions}? help · Tab menu",
            if parts.tiles.is_some() {
                "←→↑↓"
            } else {
                "↑↓"
            }
        ),
    );
}

/// The rows each part of the fight screen gets.
struct Rows {
    /// The hand's list.
    hand: u16,
    /// The rows from the creatures to the line under the hand: the list and the blank rows a
    /// smaller hand than [`SIZING_HAND`] leaves.
    hand_area: u16,
    /// The key hints and the blank row over them (none without the room).
    hints: u16,
    /// The main button.
    main: u16,
    /// The blank rows before, between and after the creatures ([`blank_rows`]).
    blanks: (u16, u16, u16),
}

/// Sizes the fight screen's parts for `area` in `plan`.
fn rows(view: &View<'_>, parts: &Parts, plan: Plan, big: Big, area: Rect) -> Rows {
    let actual = parts.height(view, plan, big, area.width);
    let pad = parts.pad_rows(plan, big, area.width);
    // Laid out for a hand of at least [`SIZING_HAND`] cards, so the creatures above and End
    // Turn below stay put while cards leave the hand; the rows they leave stay blank.
    let spare = area.height.saturating_sub(actual + pad);
    let hint_rows = if spare >= 2 { 2 } else { 0 };
    let spare = spare - hint_rows;
    // When even the tightest plan does not fit, the hand gives up rows (it scrolls to the
    // focused card) so that End Turn stays on the screen.
    let closed = parts.closed(plan, area.width);
    let overflow = actual > area.height;
    let hand_rows = if overflow {
        closed
            .saturating_sub(actual - area.height)
            .max(closed.min(2))
    } else {
        parts.hand_rows(plan, area.width, spare + pad)
    };
    let hand_area = if overflow {
        hand_rows
    } else {
        hand_rows.max(closed + pad)
    };
    let blanks = blank_rows(
        view,
        plan,
        area.width,
        spare.saturating_sub(hand_area.saturating_sub(closed + pad)),
    );
    // The hand, its line and the main button keep their rows whatever the height: the
    // creatures are drawn in the rows over them, cut when even those do not fit.
    let main_rows = match (&parts.main, plan.big_button()) {
        (None, _) => 0,
        (Some(_), true) => scale::primary_rows(),
        (Some(_), false) => 1,
    };
    Rows {
        hand: hand_rows,
        hand_area,
        hints: hint_rows,
        main: main_rows,
        blanks,
    }
}

/// The roomiest layout that fits `area` ([`candidates`]), with the fight's parts built for it;
/// the tightest plan when none does.
fn layout(view: &View<'_>, area: Rect) -> Option<(Plan, Big, Parts)> {
    let main = hand::main_button(view);
    let message = message_rows(view, area.width);
    candidates().into_iter().find_map(|(plan, big)| {
        // A big screen's roomiest plans lay the hand out as card tiles (not a card choice).
        let tiles = (big.tall_cards && view.choice.is_none() && area.width >= TILE_COLUMNS)
            .then(|| hand::tiles(view));
        let parts = Parts {
            hand: hand::buttons(view, big),
            tiles,
            main: main.clone(),
            message,
        };
        let fits = parts.height(view, plan, big, area.width)
            + parts.pad_rows(plan, big, area.width)
            <= area.height;
        (fits || plan == Plan::TIGHTEST).then_some((plan, big, parts))
    })
}

/// The blank rows before the enemies, between them and after them with `spare` rows to give:
/// rows still spare widen the blank rows over and under the enemies, a little; a plan without
/// blank rows gets back what the spare rows pay for: between the enemies first, then under
/// them, then over them.
fn blank_rows(view: &View<'_>, plan: Plan, width: u16, mut spare: u16) -> (u16, u16, u16) {
    if plan.gaps() {
        let gap = (1 + spare / 2).min(MAX_GAP);
        return (gap, 1, gap);
    }
    let blocks = flow::coord(enemies::blocks(view, plan, width).saturating_sub(1));
    let between = u16::from(blocks > 0 && spare >= blocks);
    spare -= between * blocks;
    let after = spare.min(1);
    spare -= after;
    (spare.min(1), between, after)
}

/// What the enemy turn or your last play did, in one grey line over the hand, cut with `…`
/// where it does not fit (`l` has the rest). The keys are in `?`.
fn draw_last(frame: &mut Frame<'_>, row: Rect, view: &View<'_>) {
    if row.is_empty() {
        return;
    }
    let Some(text) = view.last else {
        return;
    };
    let text = flow::truncated(&text.text(), usize::from(row.width));
    centred(
        frame,
        row,
        &[Span::styled(text, Style::new().fg(Color::Gray))],
    );
}

/// What the line under the hand says while something is half chosen, in place of the energy:
/// what to do next, in words. A card choice's prompt, when the hand's frame carries it
/// (`titled`), is not repeated here.
fn prompt(view: &View<'_>, titled: bool) -> Option<String> {
    if let Some(title) = view.title {
        let title = title.text();
        if let Some(what) = title.strip_suffix(PICK_A_TARGET) {
            let targets = view.enemies.iter().filter(|e| e.target.is_some()).count()
                + view.pets.iter().filter(|p| p.target.is_some()).count();
            let pick = if targets > 1 {
                format!("1-{}, or ←→ Enter · Esc", view.enemies.len())
            } else {
                "Enter · Esc".to_owned()
            };
            return Some(format!("{what}{pick}"));
        }
        return Some(format!("{title} · Esc"));
    }
    match view.choice {
        Some(_) if titled => None,
        Some(choice) => Some(choice.prompt.text()),
        None => None,
    }
}

/// The end of the model's title while a card or potion waits for a target.
const PICK_A_TARGET: &str = "pick a target";

/// The rows the line under the hand takes: two for a long card choice prompt.
fn message_rows(view: &View<'_>, width: u16) -> u16 {
    let long = crate::ui::notice(view.app).is_none()
        && prompt(view, false).is_some_and(|p| flow::width(&p) > usize::from(width));
    1 + u16::from(long)
}

/// The line under the hand: the screens' notice, what to do next while a card or potion waits
/// for its target or a card choice is open, else the energy (and stars) and the piles, each
/// pile a button with its key printed when there is room (`a Draw 6 · s Discard 7`).
fn draw_message(frame: &mut Frame<'_>, area: Rect, view: &View<'_>, titled: bool, hits: &mut Hits) {
    if area.is_empty() {
        return;
    }
    let app = view.app;
    let text = crate::ui::notice(app)
        .or_else(|| prompt(view, titled).map(|p| Span::styled(p, style(Role::Key))));
    if let Some(text) = text {
        let mut lines = flow::wrap(&Line::from(text), usize::from(area.width));
        lines.truncate(usize::from(area.height));
        frame.render_widget(Paragraph::new(lines), area);
        return;
    }
    let p = view.you;
    let mut f = Flow::new(area.width);
    f.token(
        Span::styled(
            format!("ϟ {}/{} Energy", p.energy, p.max_energy),
            style(Role::Energy).add_modifier(Modifier::BOLD),
        ),
        0,
        None,
    );
    if let Some(stars) = p.stars {
        f.token(
            Span::styled(format!("★ {stars}"), style(Role::Energy)),
            2,
            None,
        );
    }
    let piles = [
        ("a", "Draw", p.draw, Overlay::DrawPile),
        ("s", "Discard", p.discard, Overlay::DiscardPile),
        ("x", "Exhaust", p.exhaust, Overlay::ExhaustPile),
    ];
    let shown: Vec<_> = piles
        .into_iter()
        .filter(|(_, _, n, overlay)| *overlay != Overlay::ExhaustPile || *n > 0)
        .collect();
    // The keys are printed when the line has room for them.
    let needed: usize = shown
        .iter()
        .map(|(_, name, n, _)| flow::width(name) + 1 + n.to_string().len() + 5)
        .sum::<usize>()
        + f.used();
    let keyed = needed <= usize::from(area.width);
    for (i, (key, name, n, overlay)) in shown.into_iter().enumerate() {
        if i > 0 {
            f.token(Span::styled("·", style(Role::Muted)), 1, None);
        }
        let target = Target::Button(UiAction::Toggle(overlay));
        let first_gap = if f.used() == 0 { 0 } else { 3 };
        if keyed {
            f.token(
                Span::styled(key, style(Role::Key)),
                if i == 0 { first_gap } else { 1 },
                Some(target),
            );
        }
        f.token(
            Span::raw(format!("{name} {n}")),
            if keyed || i > 0 { 1 } else { first_gap },
            Some(target),
        );
    }
    let mut lines = f.finish(area, hits);
    lines.truncate(1);
    frame.render_widget(Paragraph::new(lines), area);
}

/// `spans` drawn in the middle of the one-row `row`; returns where they went.
pub fn centred(frame: &mut Frame<'_>, row: Rect, spans: &[Span<'static>]) -> Rect {
    let spans = flow::fit(spans, usize::from(row.width));
    let width = flow::coord(spans.iter().map(|s| flow::width(&s.content)).sum());
    let rect = Rect {
        x: row.x + (row.width - width) / 2,
        width,
        height: 1,
        ..row
    };
    frame.render_widget(Paragraph::new(Line::from(spans)), rect);
    rect
}

/// The banner of the enemies' turn or a new round ([`motion::banner`]) on the middle of the
/// blank rows `gap` between the enemies and you, for a moment; nowhere without them.
fn draw_banner(frame: &mut Frame<'_>, scene: Rect) {
    let Some(text) = motion::banner() else {
        return;
    };
    let text = format!("  {text}  ");
    let width = flow::coord(flow::width(&text)).min(scene.width);
    if scene.height == 0 || width == 0 {
        return;
    }
    let row = Rect {
        y: scene.y + scene.height / 2,
        height: 1,
        ..column::middle(scene, width)
    };
    frame.render_widget(
        Paragraph::new(Span::styled(text, flow::highlighted(style(Role::Key)))),
        row,
    );
}

/// Reports the frame's creatures to [`motion`], which animates what they lost since the last.
fn observe(view: &View<'_>) {
    let Some(combat) = view.app.combat() else {
        return;
    };
    let seen = |id, hp, block| motion::Seen { id, hp, block };
    let creatures: Vec<motion::Seen> = view
        .enemies
        .iter()
        .map(|e| seen(e.id, e.hp, e.block))
        .chain(std::iter::once(seen(
            view.you.id,
            view.you.hp,
            view.you.block,
        )))
        .chain(view.pets.iter().map(|p| seen(p.id, p.hp, p.block)))
        .collect();
    motion::observe(
        combat.state().round,
        &creatures,
        &enemy_turn(combat.events()),
    );
}

/// The hits of the enemies' last turn, in order, from the fight's log: every damage between
/// the enemies' turn starting and the player's next one, with the monster whose move it was.
fn enemy_turn(events: &[Event]) -> Vec<motion::Step> {
    let starts = |side: Side| {
        events
            .iter()
            .rposition(|e| matches!(e, Event::TurnStarted { side: s, .. } if *s == side))
    };
    let (Some(from), Some(to)) = (starts(Side::Enemy), starts(Side::Player)) else {
        return Vec::new();
    };
    if from > to {
        return Vec::new();
    }
    let mut actor = None;
    let mut steps = Vec::new();
    for event in &events[from..to] {
        match event {
            Event::MonsterMove { monster, .. } => actor = Some(monster.0),
            Event::Damage {
                dealer,
                receiver,
                blocked,
                hp_lost,
            } if *hp_lost > 0 || *blocked > 0 => steps.push(motion::Step {
                actor: dealer.map(|d| d.0).or(actor),
                receiver: receiver.0,
                hp_lost: *hp_lost,
                blocked: *blocked,
            }),
            _ => {}
        }
    }
    steps
}

/// A creature's bar while it drains a hit ([`motion::ghost`]): filled to the HP it showed, the
/// lost cells in the preview's tint above the HP it has; `None` when nothing drains.
pub fn draining_bar(
    (id, hp, max_hp, alive): (u32, i32, i32, bool),
    width: usize,
    role: Role,
    row: BarRow,
) -> Option<Vec<Span<'static>>> {
    let shown = motion::ghost(id)?;
    Some(tinted_bar(
        (shown, max_hp, alive),
        width,
        role,
        Some((hp, theme::LOSS)),
        row,
    ))
}

/// The hit floating beside a creature's name ([`motion::hit`]), drawn just right of `name` in
/// `row` where it fits; nothing moves for it.
pub fn draw_hit(frame: &mut Frame<'_>, row: Rect, name: Rect, id: u32) {
    let Some(text) = motion::hit(id) else {
        return;
    };
    let x = name.right() + 1;
    let width = flow::coord(flow::width(&text));
    if x + width > row.right() {
        return;
    }
    frame.render_widget(
        Paragraph::new(Span::styled(
            text,
            style(Role::Attack).add_modifier(Modifier::BOLD),
        )),
        Rect { x, width, ..name },
    );
}

/// What the selected card's outcome draws into a creature's HP line: the HP left (into the
/// bar and its number), the Block left, and a note after the line, each only when the play
/// changes it or the engine cannot say.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preview {
    hp_after: Option<i32>,
    block_after: Option<i32>,
    /// The HP the play takes, shown after the line (`−6`).
    hp_lost: Option<i32>,
    /// `☠ LETHAL`, `then draws`, `no change`, `?`.
    note: Option<Note>,
}

/// A note after a previewed HP line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    /// The play kills.
    Lethal,
    /// What is shown is sure; the play then draws, shuffles or rolls, so more may follow.
    ThenDraws,
    /// The play leaves the creature as it is.
    Unchanged,
    /// The engine cannot say what the play does to it.
    Unknown,
}

impl Note {
    /// The note as a span: the lethal chip, else dim words.
    #[must_use]
    pub fn span(self) -> Span<'static> {
        match self {
            Self::Lethal => Span::styled(" ☠ LETHAL ", style(Role::Lethal)),
            Self::ThenDraws => Span::styled("then draws", style(Role::Muted)),
            Self::Unchanged => Span::styled("no change", style(Role::Muted)),
            Self::Unknown => Span::styled("?", style(Role::Unknown)),
        }
    }
}

impl Preview {
    /// What follows the HP line: the HP the play takes (`−6`), then the note.
    #[must_use]
    pub fn tail(&self) -> Vec<Span<'static>> {
        let lost = self.hp_lost.map(|hp| {
            Span::styled(
                format!("−{hp}"),
                style(Role::Attack).add_modifier(Modifier::BOLD),
            )
        });
        lost.into_iter().chain(self.note.map(Note::span)).collect()
    }

    /// What `outcome` draws into its creature's HP line.
    #[must_use]
    pub fn of(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Certain(o) | Outcome::BeforeDraw(o) if o.dies => Self {
                hp_after: Some(0),
                block_after: None,
                hp_lost: Some(o.hp_before).filter(|&hp| hp > 0),
                note: Some(Note::Lethal),
            },
            Outcome::Certain(o) | Outcome::BeforeDraw(o) => Self {
                hp_after: Some(o.hp_after.max(0)).filter(|&hp| hp != o.hp_before),
                block_after: Some(o.block_after).filter(|&b| b != o.block_before),
                hp_lost: Some(o.hp_before - o.hp_after.max(0)).filter(|&lost| lost > 0),
                note: matches!(outcome, Outcome::BeforeDraw(_)).then_some(Note::ThenDraws),
            },
            Outcome::Unchanged => Self {
                note: Some(Note::Unchanged),
                ..Self::default()
            },
            Outcome::Unknown => Self {
                note: Some(Note::Unknown),
                ..Self::default()
            },
        }
    }
}

/// `▣ 5`: a creature's Block, if it has any; with `after`, the Block the selected card
/// leaves it, lit, in the same place.
pub fn block_span(block: i32, after: Option<i32>) -> Option<Span<'static>> {
    if block <= 0 {
        return None;
    }
    let (block, look) = after.map_or_else(
        || (block, style(Role::Defend).add_modifier(Modifier::BOLD)),
        |after| (after, style(Role::Key)),
    );
    Some(Span::styled(format!("▣ {block}"), look))
}

/// Whether a creature has a power worth showing.
pub fn has_powers(powers: &[PowerRow]) -> bool {
    powers.iter().any(|p| p.shown)
}

/// A creature's visible powers as tokens (`Weak 1`), as many as fit the flow's row; `short` (a
/// shared row) cuts a name that does not fit with `…` after the row's width (`Ravenous 4` →
/// `Raven… 4`) instead of dropping it. Their text is in the info overlay (`i`).
pub fn power_tokens(f: &mut Flow, id: u32, powers: &[PowerRow], short: bool) -> usize {
    let mut hidden = 0;
    for (i, p) in powers.iter().enumerate() {
        if !p.shown {
            continue;
        }
        if hidden > 0 {
            hidden += 1;
            continue;
        }
        let name = p.name.as_str();
        let amount = format!(" {}", p.amount);
        let gap = if f.width_left() == f.width() { 0 } else { 2 };
        let room = f.width_left().saturating_sub(gap + flow::width(&amount));
        let name = if flow::width(name) <= room {
            name.to_owned()
        } else if short && room >= 4 && f.width_left() == f.width() {
            flow::truncated(name, room)
        } else {
            hidden = 1;
            continue;
        };
        let text = format!("{name}{amount}");
        let role = if p.debuff { Role::Debuff } else { Role::Buff };
        f.token(
            Span::styled(text, style(role)),
            gap,
            Some(Target::Power(CreatureId(id), i)),
        );
    }
    hidden
}

/// The columns of an HP line's numbers, ` 21/33`: the HP right-aligned to the width of the
/// maximum, so the slot keeps its width whatever the HP.
#[must_use]
pub fn hp_numbers_width(max_hp: i32) -> usize {
    1 + 2 * max_hp.max(1).to_string().len() + 1
}

/// Which row of a bar is drawn: a one-row bar, or the top or bottom row of a thick one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarRow {
    Single,
    Top,
    Bottom,
}

/// `▕████████    ▏`: the HP bar `width` cells wide in a thin outline, the HP missing left
/// unfilled (a thick bar's two rows close the outline over and under it). With `after` (what
/// the selected card leaves), the cells the play takes are a pale tint of the bar. Nothing
/// moves between the two.
pub fn bar_spans(
    hp: (i32, i32, bool),
    width: usize,
    role: Role,
    after: Option<i32>,
    row: BarRow,
) -> Vec<Span<'static>> {
    tinted_bar(hp, width, role, after.map(|a| (a, theme::LOSS)), row)
}

/// [`bar_spans`] with the HP down to `after.0` tinted `after.1` (a card's preview, or what the
/// enemies' turn takes from you).
pub fn tinted_bar(
    (hp, max_hp, alive): (i32, i32, bool),
    width: usize,
    role: Role,
    after: Option<(i32, Color)>,
    row: BarRow,
) -> Vec<Span<'static>> {
    let tint = after.map_or(theme::LOSS, |(_, tint)| tint);
    let after = after.map(|(hp, _)| hp);
    let inner = width.saturating_sub(2).max(1);
    let max = usize::try_from(max_hp.max(1)).unwrap_or(1);
    let cells = |hp: i32| {
        let hp = usize::try_from(hp.max(0)).unwrap_or(0);
        if hp == 0 {
            0
        } else {
            (hp * inner).div_ceil(max).clamp(1, inner)
        }
    };
    let filled = cells(hp);
    let left = after.map_or(filled, |hp| cells(hp).min(filled));
    let on = if alive { role } else { Role::Muted };
    let edge = style(Role::Body);
    let empty = match row {
        BarRow::Single => " ",
        BarRow::Top => "▔",
        BarRow::Bottom => "▁",
    };
    vec![
        Span::styled("▕", edge),
        Span::styled("█".repeat(left), style(on)),
        Span::styled("█".repeat(filled - left), Style::new().fg(tint)),
        Span::styled(empty.repeat(inner - filled), edge),
        Span::styled("▏", edge),
    ]
}

/// ` 21/33`: an HP line's numbers in their slot (the HP right-aligned to the maximum's width),
/// in the keys' colour when `lit` (a preview's HP left).
pub fn numbers_span(hp: i32, max_hp: i32, lit: bool) -> Span<'static> {
    let max = max_hp.max(1);
    let digits = max.to_string().len();
    let look = if lit {
        style(Role::Key)
    } else {
        Style::new().add_modifier(Modifier::BOLD)
    };
    Span::styled(format!(" {:>digits$}/{max}", hp.max(0)), look)
}

/// `▕████████    ▏ 21/33`: [`bar_spans`] (its first row), then the numbers in their slot. With
/// `after` the slot shows the HP left, lit.
pub fn hp_spans(
    (hp, max_hp, alive): (i32, i32, bool),
    width: usize,
    role: Role,
    after: Option<i32>,
    row: BarRow,
) -> Vec<Span<'static>> {
    let mut spans = bar_spans((hp, max_hp, alive), width, role, after, row);
    spans.push(numbers_span(after.unwrap_or(hp), max_hp, after.is_some()));
    spans
}
