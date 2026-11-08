//! `MapPathPruning.FindMatchingSegments` for one pruning loop: the duplicate-segment groups of a
//! map, kept current while pruning removes points and edges, without listing every path.
//!
//! Adapted from spire-mentor's `SegmentIndex` (`sts2-rs/crates/sts2-core/src/map_path_pruning.rs`,
//! MIT, © 2025-2026 Thomas Friedel; see `NOTICE`): candidates found once per loop and re-checked
//! after each prune. The output is the same as the from-scratch search's, group for group and
//! segment for segment:
//!
//! - **Keys.** A segment's key (`GenerateSegmentKey`) fixes its first and last point and the
//!   point types between; a route has one point per row. A group needs two segments of one key
//!   that do not overlap, so whose inner points differ row by row. The build finds exactly the keys
//!   that have such a pair ([`Pairs`]): two routes from a valid start step in lockstep through
//!   distinct points of equal type until they reach one point, the key's last. Then it lists each
//!   such key's segments ("runs") in the search's order. That is a handful of keys, against about
//!   a thousand that the segments of all paths form.
//! - **Paths.** Pruning only removes points and edges, so a run occurs on a path of the current
//!   map exactly when the start still reaches its first point, its edges are all still there, and
//!   its last point still reaches the boss. All of that can only become false.
//! - **Order.** The search adds segments path by path in depth-first order. Runs of one key share
//!   their first point, so the first path through each passes through the same first arrival at
//!   that point and leaves their last point the same way: their first occurrences are in the order
//!   of the routes in between, the order they were listed in.
//! - **Groups.** A group's entries are its live runs in that order, each accepted unless it
//!   overlaps an accepted one, exactly as the search adds them. Groups come out in ordinal key
//!   order (`SortedDictionary`).
//! - **Types.** Only the repair step changes point types, between two pruning loops, so the keys
//!   are fixed for the life of one index.

use super::{Id, MapPointType, Point};

/// One segment of a key that can group.
#[derive(Clone, Copy, Debug)]
struct Run {
    /// The points, as `nodes[start..end]`.
    start: usize,
    end: usize,
}

/// The candidate segments of a map, grouped by key.
#[derive(Debug)]
pub(super) struct SegmentIndex {
    /// Every run's points, back to back.
    nodes: Vec<Id>,
    runs: Vec<Run>,
    /// The runs of every key that can still form a group, in ordinal key order.
    groups: Vec<Vec<usize>>,
    /// Per point, on the current map: reached from the start, and reaches the boss.
    reached: Vec<bool>,
    reaches_boss: Vec<bool>,
    /// The points by row (rows never change while pruning), so each edge goes forward.
    by_row: Vec<Id>,
}

impl SegmentIndex {
    /// Indexes the keys that can group among the paths from `start`.
    pub(super) fn build(points: &[Point], start: Id) -> Self {
        let mut index = Self {
            nodes: Vec::new(),
            runs: Vec::new(),
            groups: Vec::new(),
            reached: Vec::new(),
            reaches_boss: Vec::new(),
            by_row: (0..points.len()).collect(),
        };
        index.by_row.sort_by_key(|&id| points[id].coord.row);
        index.refresh(points, start);
        let mut keys = Vec::new();
        for (from, point) in points.iter().enumerate() {
            if index.on_a_path(from) && is_valid_start(point) {
                Pairs {
                    points,
                    reaches_boss: &index.reaches_boss,
                    from,
                    keys: &mut keys,
                }
                .start();
            }
        }
        let mut route = Vec::with_capacity(32);
        let mut groups: Vec<(SegmentKey, Vec<usize>)> = keys
            .iter()
            .map(|&key| {
                let first = index.runs.len();
                route.clear();
                route.push(key.from);
                index.list_runs(points, key, &mut route);
                let segment = index.run(first);
                (
                    SegmentKey::new(points, segment),
                    (first..index.runs.len()).collect(),
                )
            })
            .collect();
        groups.sort_unstable_by(|a, b| a.0.bytes().cmp(b.0.bytes()));
        index.groups = groups.into_iter().map(|(_, runs)| runs).collect();
        index
    }

    /// The points of run `run`: one segment of a group from [`SegmentIndex::groups`].
    pub(super) fn run(&self, run: usize) -> &[Id] {
        let Run { start, end } = self.runs[run];
        &self.nodes[start..end]
    }

    /// `FindMatchingSegments` on the current map (whose start is `start`): the groups of at
    /// least two non-overlapping segments with one key, in key order, each in the search's order,
    /// as runs ([`SegmentIndex::run`]).
    pub(super) fn groups(&mut self, points: &[Point], start: Id, out: &mut Groups) {
        out.runs.clear();
        out.ends.clear();
        self.refresh(points, start);
        let mut groups = std::mem::take(&mut self.groups);
        groups.retain_mut(|group| {
            group.retain(|&run| self.is_live(points, run));
            if group.len() < 2 {
                return false;
            }
            let from = out.runs.len();
            for &run in group.iter() {
                let nodes = self.run(run);
                if !out.runs[from..]
                    .iter()
                    .any(|&other| super::overlapping(self.run(other), nodes))
                {
                    out.runs.push(run);
                }
            }
            if out.runs.len() - from > 1 {
                out.ends.push(out.runs.len());
            } else {
                out.runs.truncate(from);
            }
            true
        });
        self.groups = groups;
    }

    /// Recomputes which points the start reaches and which reach the boss.
    fn refresh(&mut self, points: &[Point], start: Id) {
        self.reaches_boss.clear();
        self.reaches_boss.resize(points.len(), false);
        for &id in self.by_row.iter().rev() {
            self.reaches_boss[id] = points[id].kind == MapPointType::Boss
                || points[id]
                    .children
                    .iter()
                    .any(|&child| self.reaches_boss[child]);
        }
        self.reached.clear();
        self.reached.resize(points.len(), false);
        self.reached[start] = true;
        for &id in &self.by_row {
            if self.reached[id] {
                for &child in next_points(&points[id]) {
                    self.reached[child] = true;
                }
            }
        }
    }

    /// Is `id` on a path from the start to the boss?
    fn on_a_path(&self, id: Id) -> bool {
        self.reached[id] && self.reaches_boss[id]
    }

    /// Does run `run` still occur on a path, as a candidate (valid start and end)?
    fn is_live(&self, points: &[Point], run: usize) -> bool {
        let nodes = self.run(run);
        let (from, to) = (nodes[0], nodes[nodes.len() - 1]);
        self.reached[from]
            && self.reaches_boss[to]
            && is_valid_start(&points[from])
            && is_valid_end(&points[to])
            && nodes
                .windows(2)
                .all(|edge| points[edge[0]].children.contains(&edge[1]))
    }

    /// Adds the runs of `key` that extend `route`, in the search's order.
    fn list_runs(&mut self, points: &[Point], key: Key, route: &mut Vec<Id>) {
        let len = route.len();
        let id = route[len - 1];
        if len == key.len {
            if id == key.to {
                let start = self.nodes.len();
                self.nodes.extend_from_slice(route);
                self.runs.push(Run {
                    start,
                    end: self.nodes.len(),
                });
            }
            return;
        }
        for &child in next_points(&points[id]) {
            if self.reaches_boss[child] && type_bits(points[child].kind) == key.kind(len) {
                route.push(child);
                self.list_runs(points, key, route);
                route.pop();
            }
        }
    }
}

/// The points a path goes on to from `point`: none from a boss (the search stops there, so a
/// second boss is on no path).
fn next_points(point: &Point) -> &[Id] {
    if point.kind == MapPointType::Boss {
        &[]
    } else {
        &point.children
    }
}

/// The groups of one [`SegmentIndex::groups`] call: each a list of runs, back to back.
#[derive(Debug, Default)]
pub(super) struct Groups {
    runs: Vec<usize>,
    /// The end of each group in `runs`.
    ends: Vec<usize>,
}

impl Groups {
    pub(super) const fn len(&self) -> usize {
        self.ends.len()
    }

    /// The runs of group `group`.
    pub(super) fn group_mut(&mut self, group: usize) -> &mut [usize] {
        let start = group.checked_sub(1).map_or(0, |prev| self.ends[prev]);
        &mut self.runs[start..self.ends[group]]
    }
}

/// A key that can group: a first and last point, and the number of points and their types
/// between (four bits each, the first point's highest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    from: Id,
    to: Id,
    len: usize,
    types: u128,
}

impl Key {
    /// The type of the route's point `i`.
    const fn kind(self, i: usize) -> u128 {
        self.types >> (4 * (self.len - 1 - i)) & 0xF
    }
}

/// The search for the keys that can group, from one valid start: pairs of routes through
/// distinct points of equal type, row by row, until they meet.
struct Pairs<'a> {
    points: &'a [Point],
    reaches_boss: &'a [bool],
    from: Id,
    keys: &'a mut Vec<Key>,
}

impl Pairs<'_> {
    fn start(&mut self) {
        let points = self.points;
        let children = next_points(&points[self.from]);
        let types = type_bits(points[self.from].kind);
        for (i, &a) in children.iter().enumerate() {
            for &b in &children[i + 1..] {
                self.step(a, b, types, 1);
            }
        }
    }

    /// Extends the pair of routes of `len` points (their types `types`) to `a` and `b`.
    fn step(&mut self, a: Id, b: Id, types: u128, len: usize) {
        let points = self.points;
        // No path continues through a point that does not reach the boss.
        if !self.reaches_boss[a] || !self.reaches_boss[b] || points[a].kind != points[b].kind {
            return;
        }
        let types = types << 4 | type_bits(points[a].kind);
        if a == b {
            // The routes meet: a merge, so a valid end, at least three points on.
            let key = Key {
                from: self.from,
                to: a,
                len: len + 1,
                types,
            };
            if is_valid_end(&points[a]) && !self.keys.contains(&key) {
                self.keys.push(key);
            }
            return;
        }
        for &next_a in next_points(&points[a]) {
            for &next_b in next_points(&points[b]) {
                self.step(next_a, next_b, types, len + 1);
            }
        }
    }
}

/// `GenerateSegmentKey`: `"{first.col},{first.row}-{last.col},{last.row}-"` (just `"{first.row}-"`
/// for the start point) and the point types as numbers, comma-separated, as ASCII bytes. Their
/// byte order is the game's ordinal string order.
#[derive(Debug)]
struct SegmentKey {
    /// `"c,rr-c,rr-"` and at most 18 types below 16, so 10 + 18 * 3 bytes at most.
    bytes: [u8; 64],
    len: usize,
}

impl SegmentKey {
    fn new(points: &[Point], segment: &[Id]) -> Self {
        let mut key = Self {
            bytes: [0; 64],
            len: 0,
        };
        let first = points[segment[0]].coord;
        let last = points[segment[segment.len() - 1]].coord;
        if first.row != 0 {
            key.push_number(first.col);
            key.push(b',');
        }
        key.push_number(first.row);
        key.push(b'-');
        key.push_number(last.col);
        key.push(b',');
        key.push_number(last.row);
        key.push(b'-');
        for (i, &id) in segment.iter().enumerate() {
            if i > 0 {
                key.push(b',');
            }
            key.push_number(points[id].kind.index());
        }
        key
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    const fn push(&mut self, byte: u8) {
        self.bytes[self.len] = byte;
        self.len += 1;
    }

    /// Appends `n` in decimal, as `int.ToString()` writes it.
    fn push_number(&mut self, mut n: usize) {
        // Map rows, columns and point types: one or two digits.
        if let Ok(small) = u8::try_from(n)
            && small < 100
        {
            if small >= 10 {
                self.push(b'0' + small / 10);
            }
            self.push(b'0' + small % 10);
            return;
        }
        let mut digits = [0; 20];
        let mut len = 0;
        loop {
            // A digit is below 10.
            digits[len] = b'0' + u8::try_from(n % 10).unwrap_or_default();
            len += 1;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        for &digit in digits[..len].iter().rev() {
            self.push(digit);
        }
    }
}

/// `IsValidSegmentStartMapPoint`: a branch, or the start point.
fn is_valid_start(point: &Point) -> bool {
    point.children.len() > 1 || point.coord.row == 0
}

/// `IsValidSegmentEndMapPoint`: a merge.
fn is_valid_end(point: &Point) -> bool {
    point.parents.len() >= 2
}

/// A point type as the 4 bits a route key packs per point.
#[expect(
    clippy::as_conversions,
    reason = "a fieldless enum's discriminant, below 16"
)]
const fn type_bits(kind: MapPointType) -> u128 {
    kind as u128
}
