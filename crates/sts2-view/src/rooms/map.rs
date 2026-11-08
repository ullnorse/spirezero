//! The act map as a screen: the grid with its paths, the boss and what lies ahead, the last
//! room's closing text or receipt, and the nodes the player can travel to as rows.

use std::collections::{BTreeSet, HashSet};

use sts2_core::run::map::COLUMNS;
use sts2_core::run::{MapCoord, MapPointType, Run, RunAction, RunState};

use super::{Paging, View, finished_event, key_of, plain, span};
use crate::describe::Describer;
use crate::menu::MenuItem;
use crate::model::{Block, Button, ButtonId, MapCell, MapEdge, MapGrid, MapKind, MapRange, Screen};
use crate::names;
use crate::text::{Rich, RichSpan, Role};

/// The most lines of closing text or receipt under the grid.
const NOTE_LINES: usize = 4;

/// The room kinds the legend names, in its order, with the word for each.
const KINDS: [(MapPointType, &str); 6] = [
    (MapPointType::Monster, "monster"),
    (MapPointType::Elite, "elite"),
    (MapPointType::Unknown, "unknown"),
    (MapPointType::Shop, "shop"),
    (MapPointType::Treasure, "treasure"),
    (MapPointType::RestSite, "rest"),
];

/// The key of a room kind, as [`MapCell::room`] holds it.
const fn room_key(kind: MapPointType) -> &'static str {
    match kind {
        MapPointType::Unassigned => "Unassigned",
        MapPointType::Unknown => "Unknown",
        MapPointType::Shop => "Shop",
        MapPointType::Treasure => "Treasure",
        MapPointType::RestSite => "RestSite",
        MapPointType::Monster => "Monster",
        MapPointType::Elite => "Elite",
        MapPointType::Boss => "Boss",
        MapPointType::Ancient => "Ancient",
    }
}

/// The letter a room is drawn with.
const fn glyph(kind: MapPointType) -> char {
    match kind {
        MapPointType::Unassigned => ' ',
        MapPointType::Unknown => '?',
        MapPointType::Shop => '$',
        MapPointType::Treasure => 'T',
        MapPointType::RestSite => 'R',
        MapPointType::Monster => 'M',
        MapPointType::Elite => 'E',
        MapPointType::Boss => 'B',
        MapPointType::Ancient => 'A',
    }
}

/// A room's look by kind: fights as attacks, rests as HP, shops and treasure as gold, unknown
/// rooms as debuffs.
const fn role_of(kind: MapPointType) -> Role {
    match kind {
        MapPointType::Elite | MapPointType::Boss => Role::Attack,
        MapPointType::RestSite => Role::Heal,
        MapPointType::Shop | MapPointType::Treasure => Role::Energy,
        MapPointType::Unknown => Role::Debuff,
        _ => Role::Plain,
    }
}

fn byte(n: usize) -> u8 {
    u8::try_from(n).unwrap_or(u8::MAX)
}

fn pos(coord: MapCoord) -> (u8, u8) {
    (byte(coord.col), byte(coord.row))
}

/// The nodes the player can still reach from where they stand, following the paths up.
fn ahead(state: &RunState) -> HashSet<MapCoord> {
    let mut seen = HashSet::new();
    let mut stack = vec![state.coord().unwrap_or_else(|| state.map.start().coord)];
    while let Some(coord) = stack.pop() {
        for child in &state
            .map
            .point(coord)
            .map(|p| p.children)
            .unwrap_or_default()
        {
            if seen.insert(child) {
                stack.push(child);
            }
        }
    }
    seen
}

/// One node of a map as the range pass sees it: which of the legend's kinds it is (an index
/// into [`KINDS`]; `None` for the start and the boss) and where it leads.
struct RangeNode {
    kind: Option<usize>,
    children: Vec<usize>,
}

/// Per kind, the fewest and the most rooms over a set of paths.
type Counts = Vec<MapRange>;

/// Adds `b` to `a`, kind by kind.
fn plus(a: &Counts, b: &Counts) -> Counts {
    a.iter()
        .zip(b)
        .map(|(a, b)| MapRange {
            min: a.min.saturating_add(b.min),
            max: a.max.saturating_add(b.max),
        })
        .collect()
}

/// The kind-wise fewest and most over alternative `options` (each kind on its own: the path
/// with the fewest elites need not be the one with the fewest rests).
fn either(options: impl Iterator<Item = Counts>, kinds: usize) -> Counts {
    options
        .reduce(|a, b| {
            a.iter()
                .zip(&b)
                .map(|(a, b)| MapRange {
                    min: a.min.min(b.min),
                    max: a.max.max(b.max),
                })
                .collect()
        })
        .unwrap_or_else(|| vec![MapRange::default(); kinds])
}

/// The path pass over a graph: memoised counts after and before each node.
struct Pass<'a> {
    nodes: &'a [RangeNode],
    starts: &'a [usize],
    kinds: usize,
    parents: Vec<Vec<usize>>,
    after: Vec<Option<Counts>>,
    before: Vec<Option<Counts>>,
    /// Which of `before` are worked out (a node no start leads to stays `None`).
    before_done: Vec<bool>,
}

impl Pass<'_> {
    fn own(&self, n: usize) -> Counts {
        (0..self.kinds)
            .map(|k| {
                let one = u8::from(self.nodes[n].kind == Some(k));
                MapRange { min: one, max: one }
            })
            .collect()
    }

    /// The rooms past `n` on its paths to the end, `n` not counted.
    fn after(&mut self, n: usize) -> Counts {
        if let Some(done) = &self.after[n] {
            return done.clone();
        }
        let children = self.nodes[n].children.clone();
        let options: Vec<Counts> = children
            .into_iter()
            .map(|c| plus(&self.own(c), &self.after(c)))
            .collect();
        let counts = either(options.into_iter(), self.kinds);
        self.after[n] = Some(counts.clone());
        counts
    }

    /// The rooms from the starts up to `n`, `n` counted; `None` when no start leads to it.
    fn before(&mut self, n: usize) -> Option<Counts> {
        if self.before_done[n] {
            return self.before[n].clone();
        }
        let counts = if self.starts.contains(&n) {
            Some(self.own(n))
        } else {
            let parents = self.parents[n].clone();
            let options: Vec<Counts> = parents.into_iter().filter_map(|p| self.before(p)).collect();
            (!options.is_empty())
                .then(|| plus(&self.own(n), &either(options.into_iter(), self.kinds)))
        };
        self.before[n].clone_from(&counts);
        self.before_done[n] = true;
        counts
    }
}

/// What every path from `starts` (the rooms the player can enter next) to the end of the
/// graph passes, and what the paths through each node pass. The graph is acyclic; a node's
/// `through` counts itself and is `None` when no path from `starts` reaches it.
fn path_ranges(
    nodes: &[RangeNode],
    starts: &[usize],
    kinds: usize,
) -> (Counts, Vec<Option<Counts>>) {
    let mut parents = vec![Vec::new(); nodes.len()];
    for (n, node) in nodes.iter().enumerate() {
        for &c in &node.children {
            parents[c].push(n);
        }
    }
    let mut pass = Pass {
        nodes,
        starts,
        kinds,
        parents,
        after: vec![None; nodes.len()],
        before: vec![None; nodes.len()],
        before_done: vec![false; nodes.len()],
    };
    let options: Vec<Counts> = starts
        .iter()
        .map(|&s| plus(&pass.own(s), &pass.after(s)))
        .collect();
    let total = either(options.into_iter(), kinds);
    let through = (0..nodes.len())
        .map(|n| pass.before(n).map(|f| plus(&f, &pass.after(n))))
        .collect();
    (total, through)
}

/// The ranges over every path from where the player stands, and per node (by its index in
/// `coords`) over the paths through it.
fn map_ranges(
    state: &RunState,
) -> (
    std::collections::HashMap<MapCoord, usize>,
    Counts,
    Vec<Option<Counts>>,
) {
    let map = &state.map;
    let boss = map.boss().coord;
    let mut index = std::collections::HashMap::new();
    let mut nodes = Vec::new();
    for row in 0..=boss.row {
        for col in 0..COLUMNS {
            let coord = MapCoord { col, row };
            if map.point(coord).is_some() {
                index.insert(coord, nodes.len());
                nodes.push(coord);
            }
        }
    }
    let nodes: Vec<RangeNode> = nodes
        .iter()
        .filter_map(|&c| map.point(c))
        .map(|p| RangeNode {
            kind: KINDS.iter().position(|&(k, _)| k == p.kind),
            children: p
                .children
                .iter()
                .filter_map(|c| index.get(&c).copied())
                .collect(),
        })
        .collect();
    let here = state.coord().unwrap_or_else(|| map.start().coord);
    let starts: Vec<usize> = map
        .point(here)
        .map(|p| {
            p.children
                .iter()
                .filter_map(|c| index.get(&c).copied())
                .collect()
        })
        .unwrap_or_default();
    let (total, through) = path_ranges(&nodes, &starts, KINDS.len());
    (index, total, through)
}

/// The act map as a grid; `items` are the screen's choices, whose enabled ones are numbered
/// (a node travelled to by its number is `reachable` with that number as its button).
#[must_use]
pub fn grid(run: &Run, items: &[MenuItem]) -> MapGrid {
    let state = run.state();
    let map = &state.map;
    let current = state.coord();
    let reachable_from_here = ahead(state);
    let mut numbers = Vec::new();
    for (n, item) in items.iter().filter(|i| i.enabled).enumerate() {
        if let RunAction::ChooseNode(coord) = item.action {
            numbers.push((coord, byte_id(n + 1)));
        }
    }
    let boss = map.boss().coord;
    let (index, total, through) = map_ranges(state);
    let mut cells = Vec::new();
    let mut edges = Vec::new();
    for row in 0..boss.row {
        for col in 0..COLUMNS {
            let coord = MapCoord { col, row };
            let Some(point) = map.point(coord) else {
                continue;
            };
            let button = numbers.iter().find(|(c, _)| *c == coord).map(|(_, id)| *id);
            cells.push(MapCell {
                x: byte(col),
                y: byte(row),
                room: room_key(point.kind).to_owned(),
                reachable: button.is_some(),
                visited: state.visited.contains(&coord),
                glyph: glyph(point.kind),
                role: role_of(point.kind),
                ahead: reachable_from_here.contains(&coord) || current.is_none(),
                button,
                through: index
                    .get(&coord)
                    .and_then(|&i| through[i].clone())
                    .unwrap_or_default(),
            });
            edges.extend(point.children.iter().map(|to| MapEdge {
                from: pos(coord),
                to: pos(to),
            }));
        }
    }
    let kinds = KINDS
        .iter()
        .enumerate()
        .map(|(i, &(kind, word))| MapKind {
            range: total[i],
            room: room_key(kind).to_owned(),
            glyph: glyph(kind),
            word: word.to_owned(),
            role: role_of(kind),
            ahead: u32::try_from(
                reachable_from_here
                    .iter()
                    .filter(|c| map.point(**c).is_some_and(|p| p.kind == kind))
                    .count(),
            )
            .unwrap_or(0),
        })
        .collect();
    MapGrid {
        width: byte(COLUMNS),
        height: byte(boss.row + 1),
        cells,
        edges,
        you: current.map(pos),
        boss: names::title_case(state.act().boss.id),
        boss_art: state.act().boss.id.to_owned(),
        area: state.act().act.id.to_owned(),
        boss_at: pos(boss),
        boss_button: numbers.iter().find(|(c, _)| *c == boss).map(|(_, id)| *id),
        kinds,
        act: byte(state.act_index + 1),
        easy_fights_left: byte(easy_fights_left(state)),
    }
}

/// The act's easy hallway fights still to come: its first ordinary fights are its weak
/// encounters (`NumberOfWeakEncounters`), and each fight entered, a `?` room's included, takes
/// the next. What the player knows: the rule and the fights done, not which fight is next.
fn easy_fights_left(state: &RunState) -> usize {
    let act = state.act();
    act.act
        .weak_encounters
        .saturating_sub(act.normal_encounters_visited)
}

fn byte_id(n: usize) -> ButtonId {
    ButtonId(u32::try_from(n).unwrap_or(u32::MAX))
}

/// The map screen: the grid, the closing text of the event just finished (or the room's
/// receipt), and a row per node to travel to.
#[must_use]
pub fn screen(
    run: &Run,
    describer: &Describer,
    view: &View<'_>,
    items: &[MenuItem],
    paging: &Paging,
) -> Screen {
    let state = run.state();
    let mut note = finished_event(run, describer);
    if note.is_empty() {
        note = view
            .receipt
            .iter()
            .map(|line| Rich(vec![span(Role::Muted, line.clone())]))
            .collect();
    }
    note.truncate(NOTE_LINES);
    let mut blocks = vec![Block::Map(grid(run, items))];
    if !note.is_empty() {
        blocks.push(Block::Text(note));
    }
    let mut rows = Vec::new();
    for (n, item) in items.iter().filter(|i| i.enabled).enumerate() {
        let RunAction::ChooseNode(coord) = item.action else {
            continue;
        };
        let Some(point) = state.map.point(coord) else {
            continue;
        };
        let next: Vec<String> = point
            .children
            .iter()
            .filter_map(|c| state.map.point(c))
            .map(|p| glyph(p.kind))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(String::from)
            .collect();
        let mut label = Rich(vec![
            span(role_of(point.kind), glyph(point.kind).to_string()),
            RichSpan {
                fx: None,
                role: Role::Plain,
                bold: true,
                text: format!("  {}", names::point_type(state, point.kind)),
            },
        ]);
        // A hallway fight while the act's easy ones last says so.
        if point.kind == MapPointType::Monster && easy_fights_left(state) > 0 {
            label.0.push(span(Role::Muted, " · easy".to_owned()));
        }
        rows.push(Button {
            key: key_of(&paging.key(n + 1)),
            right: (!next.is_empty())
                .then(|| Rich(vec![span(Role::Muted, format!("next {}", next.join(" ")))])),
            ..Button::row(byte_id(n + 1).0, label)
        });
    }
    blocks.push(Block::List {
        frame: false,
        rows,
        page: None,
    });
    Screen {
        top: Some(crate::topbar::build(run, describer, None)),
        title: Some(plain("Map")),
        blocks,
        ..Screen::default()
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use sts2_core::Character;
    use sts2_core::run::{Act1, RunDecision, UnlockState};

    use super::*;
    use crate::menu;
    use crate::model::Key;
    use crate::text::Localization;

    fn on_map() -> (Run, Describer) {
        let mut run = Run::new(
            Character::Ironclad,
            "12345",
            0,
            UnlockState::all(),
            Act1::Random,
        )
        .expect("run")
        .with_events();
        let describer = Describer::new(Rc::new(Localization::empty()));
        for _ in 0..50 {
            if matches!(run.pending_decision(), RunDecision::Map(_)) {
                return (run, describer);
            }
            let legal = run.legal_actions();
            let action = legal
                .iter()
                .rev()
                .find(|a| **a == RunAction::Proceed)
                .or_else(|| legal.first())
                .cloned()
                .expect("a way on");
            run.apply(action).expect("legal");
        }
        panic!("the map was not reached");
    }

    #[test]
    fn the_first_map_has_every_easy_fight_to_come() {
        let (run, describer) = on_map();
        let weak = run.state().act().act.weak_encounters;
        assert!(weak > 0);
        let items = menu::items(&run, &describer, None);
        let paging = Paging::of(&items);
        let screen = screen(&run, &describer, &View::default(), &items, &paging);
        let Some(Block::Map(grid)) = screen.blocks.first() else {
            panic!("the grid first");
        };
        assert_eq!(usize::from(grid.easy_fights_left), weak);
        let Some(Block::List { rows, .. }) = screen.blocks.last() else {
            panic!("the rows last");
        };
        for row in rows {
            let label = row.label.text();
            assert_eq!(
                label.ends_with(" · easy"),
                label.contains("Monster"),
                "{label}: a fight says it is an easy one"
            );
        }
    }

    #[test]
    fn the_map_is_a_grid_with_numbered_travel_rows() {
        let (run, describer) = on_map();
        let items = menu::items(&run, &describer, None);
        let paging = Paging::of(&items);
        let screen = screen(&run, &describer, &View::default(), &items, &paging);
        let Some(Block::Map(grid)) = screen.blocks.first() else {
            panic!("the grid first");
        };
        assert!(grid.cells.iter().filter(|c| c.reachable).all(|c| c.ahead));
        assert!(!grid.edges.is_empty() && !grid.boss.is_empty());
        assert_eq!(grid.width, 7);
        let Some(Block::List { rows, .. }) = screen.blocks.last() else {
            panic!("the rows last");
        };
        assert_ne!(rows.len(), 0);
        let reachable: Vec<_> = grid.cells.iter().filter(|c| c.reachable).collect();
        assert_eq!(reachable.len(), rows.len());
        for (n, row) in rows.iter().enumerate() {
            assert_eq!(row.id.0 as usize, n + 1);
            assert_eq!(row.key, Some(Key::Digit(u8::try_from(n + 1).expect("few"))));
        }
        assert!(reachable.iter().all(|c| c.button.is_some()));
    }

    const MONSTER: usize = 0;
    const ELITE: usize = 1;
    const REST: usize = 5;

    /// start(0) -> a monster(1), b elite(2); a -> c rest(3), d elite(4); b -> d; c, d -> end(5).
    /// Paths: a c (M R), a d (M E), b d (E E).
    fn diamond() -> Vec<RangeNode> {
        let node = |kind, children: &[usize]| RangeNode {
            kind,
            children: children.to_vec(),
        };
        vec![
            node(None, &[1, 2]),
            node(Some(MONSTER), &[3, 4]),
            node(Some(ELITE), &[4]),
            node(Some(REST), &[5]),
            node(Some(ELITE), &[5]),
            node(None, &[]),
        ]
    }

    fn span(counts: &Counts, kind: usize) -> (u8, u8) {
        (counts[kind].min, counts[kind].max)
    }

    #[test]
    fn the_ranges_cover_every_path_to_the_boss() {
        let nodes = diamond();
        let (total, _) = path_ranges(&nodes, &[1, 2], 6);
        assert_eq!(span(&total, MONSTER), (0, 1));
        assert_eq!(span(&total, ELITE), (0, 2));
        assert_eq!(span(&total, REST), (0, 1));
        assert_eq!(span(&total, 3), (0, 0));
    }

    #[test]
    fn the_ranges_through_a_node_keep_only_its_paths() {
        let nodes = diamond();
        let (_, through) = path_ranges(&nodes, &[1, 2], 6);
        let rest = through[3].as_ref().expect("on a path");
        assert_eq!(span(rest, MONSTER), (1, 1));
        assert_eq!(span(rest, REST), (1, 1));
        assert_eq!(span(rest, ELITE), (0, 0));
        let d = through[4].as_ref().expect("on a path");
        assert_eq!(span(d, ELITE), (1, 2));
        assert_eq!(span(d, MONSTER), (0, 1));
        let b = through[2].as_ref().expect("on a path");
        assert_eq!(span(b, ELITE), (2, 2));
        assert!(through[0].is_none(), "the start is behind");
    }

    #[test]
    fn the_ranges_start_from_where_the_player_stands() {
        let nodes = diamond();
        // Standing on the monster: next rooms are c and d.
        let (total, through) = path_ranges(&nodes, &[3, 4], 6);
        assert_eq!(span(&total, REST), (0, 1));
        assert_eq!(span(&total, ELITE), (0, 1));
        assert_eq!(span(&total, MONSTER), (0, 0));
        assert!(through[1].is_none() && through[2].is_none());
    }

    #[test]
    fn a_real_maps_ranges_hold_its_paths() {
        let (run, describer) = on_map();
        let items = menu::items(&run, &describer, None);
        let map = grid(&run, &items);
        assert!(map.kinds.iter().all(|k| k.range.min <= k.range.max));
        assert!(map.kinds.iter().any(|k| k.range.max > 0));
        for cell in map.cells.iter().filter(|c| c.reachable) {
            assert_eq!(cell.through.len(), map.kinds.len());
            for (k, kind) in cell.through.iter().zip(&map.kinds) {
                assert!(kind.range.min <= k.min && k.max <= kind.range.max);
            }
        }
    }

    #[test]
    fn the_ahead_counts_follow_the_player() {
        let (mut run, describer) = on_map();
        let items = menu::items(&run, &describer, None);
        let before: u32 = grid(&run, &items).kinds.iter().map(|k| k.ahead).sum();
        let first = items
            .iter()
            .find(|i| i.enabled)
            .map(|i| i.action.clone())
            .expect("a node");
        run.apply(first).expect("legal");
        // Play on to the next map.
        let (after, here) = loop {
            if matches!(run.pending_decision(), RunDecision::Map(_)) {
                let items = menu::items(&run, &describer, None);
                break (grid(&run, &items), run.state().coord());
            }
            let legal = run.legal_actions();
            let action = legal
                .iter()
                .rev()
                .find(|a| **a == RunAction::Proceed)
                .or_else(|| legal.first())
                .cloned()
                .expect("a way on");
            run.apply(action).expect("legal");
        };
        assert_eq!(after.you, here.map(pos));
        let now: u32 = after.kinds.iter().map(|k| k.ahead).sum();
        assert!(now < before);
        assert!(after.cells.iter().any(|c| !c.ahead));
    }
}
