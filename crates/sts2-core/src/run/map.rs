//! Act map generation (Overgrowth, Hive, Glory): a port of `StandardActMap`, `MapPathPruning` and
//! `MapPostProcessing` (`ref/decomp/MegaCrit.Sts2.Core.Map/`), single player.
//!
//! The generator works on an arena of points because the game's graph is cyclic by reference
//! (parent and child sets). The game's `HashSet<MapPoint>` enumerates in insertion order (the
//! sets are only ever removed from once generation is over), so every parent and child set is a
//! `Vec` in insertion order. The finished [`ActMap`] is plain data keyed by [`MapCoord`].
//!
//! The map is drawn from its own stream `new Rng(seed, "act_<n>_map")`, which is not a
//! [`RunRngSet`] member and has not been touched when the map is built, and it does not read the
//! unlock state.
//!
//! The duplicate-segment search of the pruning step is incremental ([`segments`]); the rest
//! follows the game's code.

mod segments;

use std::collections::VecDeque;
use std::fmt;

use super::ascension::AscensionLevel;
use crate::rng::{Rng, RunRngSet};
use crate::shuffle::{stable_shuffle, stable_shuffle_by, unstable_shuffle};
use segments::{Groups, SegmentIndex};
use smallvec::SmallVec;

/// Grid width (`StandardActMap._mapWidth`).
pub const COLUMNS: usize = 7;
/// The tallest grid ported (Overgrowth's `BaseNumberOfRooms + 1`).
const MAX_ROWS: usize = 16;
/// Random paths walked from row 1 (`StandardActMap._iterations`).
const PATHS: usize = 7;
const MAX_PRUNE_ITERATIONS: u32 = 50;
/// The treasure row is `GetRowCount() - 7`.
const TREASURE_ROW_FROM_TOP: usize = 7;
/// Rows below this one may hold neither rest sites nor elites.
const FIRST_UNRESTRICTED_ROW: usize = 6;
/// Rows from `_mapLength - 3` up may not hold rest sites.
const NO_REST_ROWS_FROM_TOP: usize = 3;
const ELITES: usize = 5;
/// `ELITES × 1.6`, rounded (`(int)Math.Round(5f * 1.6f)`).
const SWARMING_ELITES: usize = 8;
const SHOPS: usize = 3;

/// The last act's index, the only one with a second boss (`RunManager.GenerateRooms`).
const LAST_ACT: usize = 2;

/// `GoldenPathActMap._middle`: the column of its path.
const GOLDEN_PATH_COLUMN: usize = 3;

/// `GoldenPathActMap._defaultPointTypes`, rows 1 to 16 (single player keeps all 16).
const GOLDEN_PATH_TYPES: [MapPointType; 16] = {
    use MapPointType::{Elite, Monster, RestSite, Shop, Treasure, Unknown};
    [
        Monster, Unknown, Monster, RestSite, Monster, RestSite, Unknown, Treasure, Unknown,
        Treasure, Unknown, Shop, Elite, RestSite, Elite, RestSite,
    ]
};

/// `MapPointTypeCounts.NumOfElites`: 5, ×1.6 from `SwarmingElites` on.
const fn elite_count(ascension: u8) -> usize {
    if AscensionLevel::SwarmingElites.is_active(ascension) {
        SWARMING_ELITES
    } else {
        ELITES
    }
}

/// A map position, ordered by column then row like the game's `MapCoord.CompareTo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapCoord {
    /// The column.
    pub col: usize,
    /// The row.
    pub row: usize,
}

/// `MapPointType`, in the game's enum order (the order is part of the pruning keys).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MapPointType {
    /// `MapPointType.Unassigned`.
    Unassigned,
    /// `MapPointType.Unknown`.
    Unknown,
    /// `MapPointType.Shop`.
    Shop,
    /// `MapPointType.Treasure`.
    Treasure,
    /// `MapPointType.RestSite`.
    RestSite,
    /// `MapPointType.Monster`.
    Monster,
    /// `MapPointType.Elite`.
    Elite,
    /// `MapPointType.Boss`.
    Boss,
    /// `MapPointType.Ancient`.
    Ancient,
}

impl_index!(MapPointType);

impl MapPointType {
    /// The C# enum member name, as the referee prints it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unassigned => "Unassigned",
            Self::Unknown => "Unknown",
            Self::Shop => "Shop",
            Self::Treasure => "Treasure",
            Self::RestSite => "RestSite",
            Self::Monster => "Monster",
            Self::Elite => "Elite",
            Self::Boss => "Boss",
            Self::Ancient => "Ancient",
        }
    }
}

/// Map generation failed where the real game throws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapError {
    /// `GenerateNextCoord`: every column choice crosses an existing edge.
    NoNextNode,
    /// `PruneDuplicateSegments`: still duplicates after 50 iterations.
    PruneDidNotConverge,
    /// `SpoilsActMap.GenerateNextCoord`: even the fallback step is more than one column away.
    InvalidStep,
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoNextNode => f.write_str("cannot find next node"),
            Self::PruneDidNotConverge => f.write_str("unable to prune matching segments"),
            Self::InvalidStep => f.write_str("fallback step exceeds adjacency"),
        }
    }
}

impl std::error::Error for MapError {}

/// One node of a finished map. `parents` and `children` are in the game's set order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapPoint {
    /// Where it is.
    pub coord: MapCoord,
    /// `PointType`.
    pub kind: MapPointType,
    /// `parents`.
    pub parents: Neighbors,
    /// `Children`.
    pub children: Neighbors,
}

/// A point's parents or children, in the game's set order: distinct points of one row, so at
/// most [`COLUMNS`]. Stored inline (a byte per column and row) so a map is one flat array.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Neighbors {
    len: u8,
    coords: [(u8, u8); COLUMNS],
}

impl Neighbors {
    /// The list of `coords`, in order.
    ///
    /// # Panics
    /// With more than [`COLUMNS`] coords, or a coord past 255.
    #[must_use]
    pub fn of(coords: &[MapCoord]) -> Self {
        let mut neighbors = Self::default();
        for &coord in coords {
            neighbors.push(coord);
        }
        neighbors
    }

    fn push(&mut self, coord: MapCoord) {
        let byte = |v: usize| u8::try_from(v).expect("map coords fit a byte");
        self.coords[usize::from(self.len)] = (byte(coord.col), byte(coord.row));
        self.len += 1;
    }

    /// The number of coords.
    #[must_use]
    #[expect(clippy::as_conversions, reason = "const fn: `From` is not const")]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether there are none.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The coord at `index`.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<MapCoord> {
        self.packed().get(index).map(|&p| unpack(p))
    }

    /// The first coord.
    #[must_use]
    pub fn first(&self) -> Option<MapCoord> {
        self.get(0)
    }

    /// Whether `coord` is one of them.
    #[must_use]
    pub fn contains(&self, coord: &MapCoord) -> bool {
        self.iter().any(|c| c == *coord)
    }

    /// The coords in order.
    pub fn iter(&self) -> NeighborIter<'_> {
        self.packed().iter().map(|&p| unpack(p))
    }

    /// The coords, in order.
    #[must_use]
    pub fn to_vec(&self) -> Vec<MapCoord> {
        self.iter().collect()
    }

    fn packed(&self) -> &[(u8, u8)] {
        &self.coords[..self.len()]
    }
}

/// [`Neighbors::iter`].
pub type NeighborIter<'a> =
    std::iter::Map<std::slice::Iter<'a, (u8, u8)>, fn(&(u8, u8)) -> MapCoord>;

impl<'a> IntoIterator for &'a Neighbors {
    type Item = MapCoord;
    type IntoIter = NeighborIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[expect(clippy::as_conversions, reason = "const fn: `From` is not const")]
const fn unpack((col, row): (u8, u8)) -> MapCoord {
    MapCoord {
        col: col as usize,
        row: row as usize,
    }
}

impl fmt::Debug for Neighbors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<const N: usize> PartialEq<[MapCoord; N]> for Neighbors {
    fn eq(&self, other: &[MapCoord; N]) -> bool {
        self.iter().eq(other.iter().copied())
    }
}

impl PartialEq<Vec<MapCoord>> for Neighbors {
    fn eq(&self, other: &Vec<MapCoord>) -> bool {
        self.iter().eq(other.iter().copied())
    }
}

impl PartialEq<[MapCoord]> for Neighbors {
    fn eq(&self, other: &[MapCoord]) -> bool {
        self.iter().eq(other.iter().copied())
    }
}

/// A generated act map: the start (Ancient) point, the boss point, the rooms between and, in the
/// last act from `DoubleBoss` on, a second boss point after the boss.
#[derive(Clone, PartialEq, Eq)]
pub struct ActMap {
    points: Vec<MapPoint>,
    start: MapPoint,
    boss: MapPoint,
    second_boss: Option<MapPoint>,
    /// Made by `SpoilsActMap`.
    spoils: bool,
}

// `spoils` is a label, not part of the map's shape: it stays out of the trace the digest hashes.
#[expect(
    clippy::missing_fields_in_debug,
    reason = "`spoils` must stay out of the digest trace"
)]
impl fmt::Debug for ActMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActMap")
            .field("points", &self.points)
            .field("start", &self.start)
            .field("boss", &self.boss)
            .field("second_boss", &self.second_boss)
            .finish()
    }
}

impl ActMap {
    /// Whether this is a `SpoilsActMap` (`BigGameHunter` regenerates standard maps only).
    #[must_use]
    pub const fn is_spoils(&self) -> bool {
        self.spoils
    }

    /// Generates the Act 1 map of the run whose streams are `rngs`.
    ///
    /// # Errors
    /// Where the game throws (see [`MapError`]); such a seed has no map in the real game.
    pub fn generate_act1(rngs: &RunRngSet, ascension: u8) -> Result<Self, MapError> {
        Self::generate(rngs, 0, ascension)
    }

    /// Generates the map of act `act_index` (0 Overgrowth or Underdocks, 1 Hive, 2 Glory) from
    /// `new Rng(seed, "act_<act_index + 1>_map")` (`StandardActMap.CreateFor`) at `ascension`.
    /// The last act gets a second boss point from `DoubleBoss` on (`ActModel.HasSecondBoss`:
    /// `GenerateRooms` gives that act a second boss encounter exactly then).
    ///
    /// # Errors
    /// Where the game throws (see [`MapError`]).
    ///
    /// # Panics
    /// For an act index past the last act.
    pub fn generate(rngs: &RunRngSet, act_index: usize, ascension: u8) -> Result<Self, MapError> {
        let shape = act_shape(act_index);
        let rng = Rng::named(rngs.seed(), &format!("act_{}_map", act_index + 1));
        let second_boss = act_index == LAST_ACT && AscensionLevel::DoubleBoss.is_active(ascension);
        let mut builder = Builder::new(rng, &shape, elite_count(ascension), Kind::Standard);
        if second_boss {
            builder.add_second_boss();
        }
        builder.build()
    }

    /// `BigGameHunter.ModifyGeneratedMap` for a `StandardActMap` or a `SpoilsActMap` (the latter
    /// from its own `spoils_map` stream; `Modifiers/BigGameHunter.cs:14-24`):
    /// the act's map again from a fresh `act_<n>_map` stream, with the unknown and rest counts of
    /// `existing` (`new MapPointTypeCounts(map)`), shops 3 and `round(elites in existing × 2.5)`
    /// elites (`Math.Round` rounds halves to even), which ignore the placement rules. Nothing is
    /// drawn for the counts.
    ///
    /// # Errors
    /// Where the game throws (see [`MapError`]).
    ///
    /// # Panics
    /// For an act index past the last act.
    pub fn generate_big_game_hunter(
        rngs: &RunRngSet,
        act_index: usize,
        ascension: u8,
        existing: &Self,
    ) -> Result<Self, MapError> {
        let count = |kind| existing.points.iter().filter(|p| p.kind == kind).count();
        // `(int)Math.Round(n * 2.5f)`: n × 5 halves, rounded to even.
        let halves = count(MapPointType::Elite) * 5;
        let elites = halves / 2 + usize::from(halves % 2 == 1 && (halves / 2) % 2 == 1);
        let counts = Counts {
            rests: count(MapPointType::RestSite),
            shops: SHOPS,
            elites,
            unknowns: count(MapPointType::Unknown),
            elites_ignore_rules: true,
        };
        let shape = act_shape(act_index);
        if existing.spoils {
            // `new SpoilsActMap(runState, counts)`: its own stream, no second boss.
            let rng = Rng::named(rngs.seed(), "spoils_map");
            return Builder::with_counts(rng, &shape, counts, Kind::Spoils).build();
        }
        let rng = Rng::named(rngs.seed(), &format!("act_{}_map", act_index + 1));
        let second_boss = act_index == LAST_ACT && AscensionLevel::DoubleBoss.is_active(ascension);
        let mut builder = Builder::with_counts(rng, &shape, counts, Kind::Standard);
        if second_boss {
            builder.add_second_boss();
        }
        builder.build()
    }

    /// Generates the hourglass map of act `act_index` that a `SpoilsMap` in the deck swaps in
    /// (`SpoilsMap.ModifyGeneratedMap` → `SpoilsActMap`): the same point counts and pruning, but
    /// drawn from `new Rng(seed, "spoils_map")`, with every path funnelled through one centred
    /// treasure point and no post-processing.
    ///
    /// # Errors
    /// Where the game throws (see [`MapError`]).
    ///
    /// # Panics
    /// For an act index past the last act.
    pub fn generate_spoils(
        rngs: &RunRngSet,
        act_index: usize,
        ascension: u8,
    ) -> Result<Self, MapError> {
        let rng = Rng::named(rngs.seed(), "spoils_map");
        Builder::new(
            rng,
            &act_shape(act_index),
            elite_count(ascension),
            Kind::Spoils,
        )
        .build()
    }

    /// `GoldenPathActMap(runState)` (`Map/GoldenPathActMap.cs:39-68`), single player: one
    /// straight path up the middle column through 16 fixed rooms, between the start (Ancient)
    /// point `(3, 0)` and the boss point `(3, 17)`. Nothing is drawn.
    #[must_use]
    pub fn golden_path() -> Self {
        let at = |row: usize| MapCoord {
            col: GOLDEN_PATH_COLUMN,
            row,
        };
        let rows = GOLDEN_PATH_TYPES.len();
        let points = GOLDEN_PATH_TYPES
            .iter()
            .enumerate()
            .map(|(i, &kind)| {
                let row = i + 1;
                MapPoint {
                    coord: at(row),
                    kind,
                    parents: Neighbors::of(&[at(row - 1)]),
                    children: Neighbors::of(&[at(row + 1)]),
                }
            })
            .collect();
        Self {
            points,
            start: MapPoint {
                coord: at(0),
                kind: MapPointType::Ancient,
                parents: Neighbors::default(),
                children: Neighbors::of(&[at(1)]),
            },
            boss: MapPoint {
                coord: at(rows + 1),
                kind: MapPointType::Boss,
                parents: Neighbors::of(&[at(rows)]),
                children: Neighbors::default(),
            },
            second_boss: None,
            spoils: false,
        }
    }

    /// Whether the map has a point at `coord` (`ActMap.HasPoint`).
    #[must_use]
    pub fn has_point(&self, coord: MapCoord) -> bool {
        self.point(coord).is_some()
    }

    /// The room points, ordered by row then column.
    #[must_use]
    pub fn points(&self) -> &[MapPoint] {
        &self.points
    }

    /// The start point `(3, 0)` (the act's Ancient).
    #[must_use]
    pub const fn start(&self) -> &MapPoint {
        &self.start
    }

    /// The boss point `(3, rooms + 1)`.
    #[must_use]
    pub const fn boss(&self) -> &MapPoint {
        &self.boss
    }

    /// The second boss point `(3, rooms + 2)`, the boss point's only child (`SecondBossMapPoint`).
    #[must_use]
    pub const fn second_boss(&self) -> Option<&MapPoint> {
        self.second_boss.as_ref()
    }

    /// The point at `coord`, including the start and boss points (`ActMap.GetPoint`).
    #[must_use]
    pub fn point(&self, coord: MapCoord) -> Option<&MapPoint> {
        if coord == self.start.coord {
            return Some(&self.start);
        }
        if coord == self.boss.coord {
            return Some(&self.boss);
        }
        if let Some(second) = self.second_boss.as_ref().filter(|p| p.coord == coord) {
            return Some(second);
        }
        self.points
            .binary_search_by_key(&(coord.row, coord.col), |p| (p.coord.row, p.coord.col))
            .ok()
            .map(|i| &self.points[i])
    }
}

type Id = usize;
type Segment = Vec<Id>;
/// Points of one row (a point's parents or children, a row's points): at most [`COLUMNS`].
type Row = SmallVec<[Id; COLUMNS]>;

const START: Id = 0;
const BOSS: Id = 1;
/// Only present when [`Builder::add_second_boss`] ran.
const SECOND_BOSS: Id = 2;

struct Point {
    coord: MapCoord,
    kind: MapPointType,
    can_be_modified: bool,
    parents: Row,
    children: Row,
}

/// The map parameters of act `act_index` (`ActModel.BaseNumberOfRooms`, `GetMapPointTypes`).
/// Underdocks shares Overgrowth's (`Underdocks.cs`).
fn act_shape(act_index: usize) -> ActShape {
    match act_index {
        // `Overgrowth`: 15 rooms, `NextGaussianInt(7, 1, 6, 7)` rests.
        0 => ActShape {
            rooms: 15,
            rests: RestDraw::Gaussian(7),
            unknown_offset: 0,
        },
        // `Hive`: 14 rooms, `NextGaussianInt(6, 1, 6, 7)` rests, one unknown less.
        1 => ActShape {
            rooms: 14,
            rests: RestDraw::Gaussian(6),
            unknown_offset: 1,
        },
        // `Glory`: 13 rooms, `NextInt(5, 7)` rests, one unknown less.
        LAST_ACT => ActShape {
            rooms: 13,
            rests: RestDraw::FiveOrSix,
            unknown_offset: 1,
        },
        _ => panic!("there is no act {act_index}"),
    }
}

/// What differs between the acts' maps (`ActModel.BaseNumberOfRooms`, `GetMapPointTypes`).
struct ActShape {
    rooms: usize,
    rests: RestDraw,
    /// Subtracted from `StandardRandomUnknownCount`.
    unknown_offset: usize,
}

/// How `GetMapPointTypes` draws the rest-site count, the map stream's first draw.
#[derive(Clone, Copy)]
enum RestDraw {
    /// `NextGaussianInt(mean, 1, 6, 7)`.
    Gaussian(i32),
    /// `NextInt(5, 7)` (Glory).
    FiveOrSix,
}

impl RestDraw {
    fn draw(self, rng: &mut Rng) -> usize {
        let rests = match self {
            Self::Gaussian(mean) => rng.next_gaussian_int(mean, 1, 6, 7),
            Self::FiveOrSix => rng.next_int_range(5, 7),
        };
        usize::try_from(rests).expect("rest count is 5 to 7")
    }
}

/// Which generator draws the paths.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `StandardActMap`.
    Standard,
    /// `SpoilsActMap`: an hourglass through one treasure point.
    Spoils,
}

/// The point-type targets (`MapPointTypeCounts`).
struct Counts {
    rests: usize,
    shops: usize,
    elites: usize,
    unknowns: usize,
    /// `MapPointTypeCounts.PointTypesThatIgnoreRules` holds `Elite` (`BigGameHunter`).
    elites_ignore_rules: bool,
}

struct Builder {
    rng: Rng,
    points: Vec<Point>,
    /// Grid height (`_mapLength`): `BaseNumberOfRooms + 1`. Rows `1..rows` are rooms; row 0
    /// only holds the start and row `rows` the boss.
    rows: usize,
    grid: [[Option<Id>; MAX_ROWS]; COLUMNS],
    start_points: Vec<Id>,
    counts: Counts,
    kind: Kind,
    has_second_boss: bool,
}

impl Builder {
    fn new(mut rng: Rng, shape: &ActShape, elites: usize, kind: Kind) -> Self {
        // `GetMapPointTypes`: rests first, then unknowns (`StandardRandomUnknownCount`).
        let rests = shape.rests.draw(&mut rng);
        let unknowns = rng.next_gaussian_int(12, 1, 10, 14);
        let counts = Counts {
            rests,
            shops: SHOPS,
            elites,
            unknowns: usize::try_from(unknowns).expect("unknown count is 10 to 14")
                - shape.unknown_offset,
            elites_ignore_rules: false,
        };
        Self::with_counts(rng, shape, counts, kind)
    }

    /// A builder for the point counts `mapPointTypeCountsOverride` gives: nothing is drawn.
    fn with_counts(rng: Rng, shape: &ActShape, counts: Counts, kind: Kind) -> Self {
        let rows = shape.rooms + 1;
        let new_point = |col, row| Point {
            coord: MapCoord { col, row },
            kind: MapPointType::Unassigned,
            can_be_modified: true,
            parents: Row::new(),
            children: Row::new(),
        };
        // Room for every point the map can have (the grid, start, boss and second boss): a
        // point is large, and growing the list copies it.
        let mut points = Vec::with_capacity(COLUMNS * MAX_ROWS + 3);
        points.extend([new_point(COLUMNS / 2, 0), new_point(COLUMNS / 2, rows)]);
        Self {
            rng,
            points,
            rows,
            grid: [[None; MAX_ROWS]; COLUMNS],
            start_points: Vec::new(),
            counts,
            kind,
            has_second_boss: false,
        }
    }

    /// `SecondBossMapPoint` at `(3, rows + 1)`, created with the start and boss points; the
    /// boss gets it as its only child once the paths are drawn, and its type is Boss. Pruning
    /// stops at the boss and post-processing only moves grid points, so neither sees it.
    fn add_second_boss(&mut self) {
        debug_assert_eq!(self.points.len(), SECOND_BOSS);
        self.points.push(Point {
            coord: MapCoord {
                col: COLUMNS / 2,
                row: self.rows + 1,
            },
            kind: MapPointType::Boss,
            can_be_modified: true,
            parents: Row::new(),
            children: Row::new(),
        });
        self.has_second_boss = true;
    }

    fn build(self) -> Result<ActMap, MapError> {
        self.build_traced().map(|(map, _)| map)
    }

    /// [`Self::build`], also returning the map stream as generation left it.
    fn build_traced(mut self) -> Result<(ActMap, Rng), MapError> {
        self.generate_paths()?;
        self.assign_point_types();
        self.prune_and_repair()?;
        if self.kind == Kind::Standard {
            self.center_grid();
            self.spread_adjacent_points();
            self.straighten_paths();
        }
        Ok((self.finish(), self.rng))
    }

    // --- graph basics ---------------------------------------------------------------------

    fn coord(&self, id: Id) -> MapCoord {
        self.points[id].coord
    }

    fn kind(&self, id: Id) -> MapPointType {
        self.points[id].kind
    }

    fn get_or_create(&mut self, col: usize, row: usize) -> Id {
        if let Some(id) = self.grid[col][row] {
            return id;
        }
        let id = self.points.len();
        self.points.push(Point {
            coord: MapCoord { col, row },
            kind: MapPointType::Unassigned,
            can_be_modified: true,
            parents: Row::new(),
            children: Row::new(),
        });
        self.grid[col][row] = Some(id);
        id
    }

    fn add_child(&mut self, parent: Id, child: Id) {
        if !self.points[parent].children.contains(&child) {
            self.points[parent].children.push(child);
        }
        if !self.points[child].parents.contains(&parent) {
            self.points[child].parents.push(parent);
        }
    }

    fn remove_child(&mut self, parent: Id, child: Id) {
        self.points[parent].children.retain(|c| *c != child);
        self.points[child].parents.retain(|p| *p != parent);
    }

    /// `ActMap.GetAllMapPoints`: grid points, column-major.
    fn all_points(&self) -> SmallVec<[Id; COLUMNS * MAX_ROWS]> {
        self.grid.iter().flatten().flatten().copied().collect()
    }

    /// `ForEachInRow`: grid points of `row`, column ascending.
    fn row_points(&self, row: usize) -> Row {
        self.grid.iter().filter_map(|col| col[row]).collect()
    }

    // --- path generation (`StandardActMap.GenerateMap`) -----------------------------------

    fn generate_paths(&mut self) -> Result<(), MapError> {
        for i in 0..PATHS {
            let col = self.next_start_col();
            let mut start = self.get_or_create(col, 1);
            if i == 1 {
                while self.start_points.contains(&start) {
                    let col = self.next_start_col();
                    start = self.get_or_create(col, 1);
                }
            }
            if !self.start_points.contains(&start) {
                self.start_points.push(start);
            }
            self.path_generate(start)?;
        }
        if self.kind == Kind::Spoils {
            self.funnel_into_treasure();
        }
        for id in self.row_points(self.rows - 1) {
            self.add_child(id, BOSS);
        }
        if self.has_second_boss {
            self.add_child(BOSS, SECOND_BOSS);
        }
        for id in self.row_points(1) {
            self.add_child(START, id);
        }
        Ok(())
    }

    /// `SpoilsActMap.GenerateHourglassMap`: one treasure point at the centre of its row, and every
    /// other point of that row (`RedirectToTreasure`) hands its edges to it. The .NET sets would
    /// put the treasure into the freed slot, but a row-7 parent's only other possible entry is
    /// the treasure itself (row 8 holds nothing else), so appending gives the same order.
    fn funnel_into_treasure(&mut self) {
        let row = self.rows - TREASURE_ROW_FROM_TOP;
        let treasure = self.get_or_create(COLUMNS / 2, row);
        self.points[treasure].kind = MapPointType::Treasure;
        self.points[treasure].can_be_modified = false;
        for stray in self.row_points(row) {
            if stray == treasure {
                continue;
            }
            for parent in self.points[stray].parents.clone() {
                self.remove_child(parent, stray);
                self.add_child(parent, treasure);
            }
            for child in self.points[stray].children.clone() {
                self.remove_child(stray, child);
                self.add_child(treasure, child);
            }
            self.grid[self.coord(stray).col][row] = None;
        }
    }

    fn next_start_col(&mut self) -> usize {
        usize::try_from(self.rng.next_int_range(0, 7)).expect("next_int_range(0, 7) is >= 0")
    }

    fn path_generate(&mut self, start: Id) -> Result<(), MapError> {
        let mut current = start;
        while self.coord(current).row < self.rows - 1 {
            let coord = match self.kind {
                Kind::Standard => self.generate_next_coord(current)?,
                Kind::Spoils => self.generate_spoils_coord(current)?,
            };
            let next = self.get_or_create(coord.col, coord.row);
            self.add_child(current, next);
            current = next;
        }
        Ok(())
    }

    fn generate_next_coord(&mut self, current: Id) -> Result<MapCoord, MapError> {
        let MapCoord { col, row } = self.coord(current);
        let mut offsets = [-1_i8, 0, 1];
        stable_shuffle(&mut offsets, &mut self.rng);
        for offset in offsets {
            let target = match offset {
                -1 => col.saturating_sub(1),
                0 => col,
                _ => (col + 1).min(COLUMNS - 1),
            };
            if !self.has_invalid_crossover(current, target) {
                return Ok(MapCoord {
                    col: target,
                    row: row + 1,
                });
            }
        }
        Err(MapError::NoNextNode)
    }

    /// `SpoilsActMap.GenerateNextCoord`: the columns are limited to a diamond around the treasure
    /// row, a point has at most 3 parents and 3 children, and the last rows before the treasure
    /// step towards the centre column without a draw.
    fn generate_spoils_coord(&mut self, current: Id) -> Result<MapCoord, MapError> {
        let MapCoord { col, row: from } = self.coord(current);
        let row = from + 1;
        let treasure_row = self.rows - TREASURE_ROW_FROM_TOP;
        let center = COLUMNS / 2;
        let (min_col, max_col) = self.spoils_columns(row);
        let mut offsets = [-1_i8, 0, 1];
        if treasure_row.saturating_sub(from) > 3 || from >= treasure_row {
            stable_shuffle(&mut offsets, &mut self.rng);
        } else {
            offsets = centred_offsets(col, center);
        }
        for offset in offsets {
            let target = match offset {
                -1 => col.saturating_sub(1),
                0 => col,
                _ => (col + 1).min(COLUMNS - 1),
            };
            if target < min_col || target > max_col || self.has_invalid_crossover(current, target) {
                continue;
            }
            let point = self.grid[target][row];
            let parent_ok = point.is_none_or(|p| {
                self.points[p].parents.contains(&current) || self.points[p].parents.len() < 3
            });
            let child_ok = self.points[current].children.len() < 3
                || point.is_some_and(|p| self.points[current].children.contains(&p));
            if parent_ok && child_ok {
                return Ok(MapCoord { col: target, row });
            }
        }
        let mut fallback = center.clamp(min_col, max_col);
        if fallback.abs_diff(col) > 1 {
            fallback = if fallback > col { col + 1 } else { col - 1 }.clamp(min_col, max_col);
        }
        if self.has_invalid_crossover(current, fallback) {
            fallback = col.clamp(min_col, max_col);
        }
        if fallback.abs_diff(col) > 1 {
            return Err(MapError::InvalidStep);
        }
        Ok(MapCoord { col: fallback, row })
    }

    /// `SpoilsActMap.GetAllowedColumnsForRow`: the hourglass is widest at the first and last row
    /// and pinched to the centre column at the treasure row.
    fn spoils_columns(&self, row: usize) -> (usize, usize) {
        let center = COLUMNS / 2;
        let treasure_row = self.rows - TREASURE_ROW_FROM_TOP;
        let from_end = (self.rows - 1).saturating_sub(row) + 1;
        let half_width = center.min(row.abs_diff(treasure_row).min(from_end.min(center)));
        (center - half_width, (center + half_width).min(COLUMNS - 1))
    }

    /// Whether stepping from `current` to column `target` would cross an edge of the point at
    /// `(target, current.row)` that goes the other way (an X).
    fn has_invalid_crossover(&self, current: Id, target: usize) -> bool {
        let MapCoord { col, row } = self.coord(current);
        if target == col {
            return false;
        }
        let Some(other) = self.grid[target][row] else {
            return false;
        };
        let other_col = self.coord(other).col;
        // `child.col - other.col == -(target - col)`, rearranged to stay unsigned.
        self.points[other]
            .children
            .iter()
            .any(|&child| self.coord(child).col + target == col + other_col)
    }

    // --- point types (`StandardActMap.AssignPointTypes`) ----------------------------------

    fn assign_point_types(&mut self) {
        for (row, kind) in [
            (self.rows - 1, MapPointType::RestSite),
            (self.rows - TREASURE_ROW_FROM_TOP, MapPointType::Treasure),
            (1, MapPointType::Monster),
        ] {
            for id in self.row_points(row) {
                self.points[id].kind = kind;
                self.points[id].can_be_modified = false;
            }
        }
        let c = &self.counts;
        let mut queue: VecDeque<MapPointType> = [
            (MapPointType::RestSite, c.rests),
            (MapPointType::Shop, c.shops),
            (MapPointType::Elite, c.elites),
            (MapPointType::Unknown, c.unknowns),
        ]
        .into_iter()
        .flat_map(|(kind, n)| std::iter::repeat_n(kind, n))
        .collect();
        self.assign_remaining_types(&mut queue);
        for id in self.all_points() {
            if self.kind(id) == MapPointType::Unassigned {
                self.points[id].kind = MapPointType::Monster;
            }
        }
        self.points[BOSS].kind = MapPointType::Boss;
        self.points[START].kind = MapPointType::Ancient;
    }

    fn assign_remaining_types(&mut self, queue: &mut VecDeque<MapPointType>) {
        for _ in 0..3 {
            if queue.is_empty() {
                break;
            }
            let mut unassigned: Vec<Id> = self
                .all_points()
                .into_iter()
                .filter(|&id| self.kind(id) == MapPointType::Unassigned)
                .collect();
            self.shuffle_by_coord(&mut unassigned);
            for id in unassigned {
                if queue.is_empty() {
                    break;
                }
                self.points[id].kind = self.next_valid_point_type(queue, id);
            }
        }
    }

    /// `List<MapPoint>.StableShuffle`: sort by `MapPoint.CompareTo` (the coordinate), then shuffle.
    fn shuffle_by_coord(&mut self, ids: &mut [Id]) {
        let points = &self.points;
        stable_shuffle_by(ids, &mut self.rng, |&a, &b| {
            points[a].coord.cmp(&points[b].coord)
        });
    }

    /// Rotates `queue` until a type fits `id`; `Unassigned` if none does.
    fn next_valid_point_type(&self, queue: &mut VecDeque<MapPointType>, id: Id) -> MapPointType {
        let mut tried = 0;
        while tried < queue.len() {
            let Some(kind) = queue.pop_front() else {
                break;
            };
            if (kind == MapPointType::Elite && self.counts.elites_ignore_rules)
                || self.is_valid_point_type(kind, id)
            {
                return kind;
            }
            queue.push_back(kind);
            tried += 1;
        }
        MapPointType::Unassigned
    }

    fn is_valid_point_type(&self, kind: MapPointType, id: Id) -> bool {
        use MapPointType::{Elite, Monster, RestSite, Shop, Treasure, Unknown};
        let point = &self.points[id];
        if point.coord.row >= self.rows - NO_REST_ROWS_FROM_TOP && kind == RestSite {
            return false;
        }
        if point.coord.row < FIRST_UNRESTRICTED_ROW && matches!(kind, RestSite | Elite) {
            return false;
        }
        // Not twice in a row, with a parent or a child.
        if matches!(kind, Elite | RestSite | Treasure | Shop)
            && point
                .parents
                .iter()
                .chain(&point.children)
                .any(|&p| self.kind(p) == kind)
        {
            return false;
        }
        // Not next to a sibling (another child of one of this point's parents).
        if matches!(kind, RestSite | Monster | Unknown | Elite | Shop) {
            let sibling_has_kind = point
                .parents
                .iter()
                .flat_map(|&parent| &self.points[parent].children)
                .any(|&sibling| sibling != id && self.kind(sibling) == kind);
            if sibling_has_kind {
                return false;
            }
        }
        true
    }

    // --- pruning (`MapPathPruning`) -------------------------------------------------------

    fn prune_and_repair(&mut self) -> Result<(), MapError> {
        for _ in 0..3 {
            self.prune_duplicate_segments()?;
            if !self.repair_pruned_point_types() {
                break;
            }
        }
        Ok(())
    }

    fn repair_pruned_point_types(&mut self) -> bool {
        let targets = [
            (MapPointType::Shop, self.counts.shops),
            (MapPointType::Elite, self.counts.elites),
            (MapPointType::RestSite, self.counts.rests),
            (MapPointType::Unknown, self.counts.unknowns),
        ];
        // No short-circuit: every type is repaired.
        targets.into_iter().fold(false, |changed, (kind, target)| {
            self.repair_point_type(kind, target) | changed
        })
    }

    fn repair_point_type(&mut self, kind: MapPointType, target: usize) -> bool {
        let present = self
            .all_points()
            .into_iter()
            .filter(|&id| self.kind(id) == kind)
            .count();
        let mut missing = target.saturating_sub(present);
        if missing == 0 {
            return false;
        }
        let mut candidates: SmallVec<[Id; COLUMNS * MAX_ROWS]> = self
            .all_points()
            .into_iter()
            .filter(|&id| self.kind(id) == MapPointType::Monster && self.points[id].can_be_modified)
            .collect();
        self.shuffle_by_coord(&mut candidates);
        let mut changed = false;
        for id in candidates {
            if missing == 0 {
                break;
            }
            if self.is_valid_point_type(kind, id) {
                self.points[id].kind = kind;
                missing -= 1;
                changed = true;
            }
        }
        changed
    }

    fn prune_duplicate_segments(&mut self) -> Result<(), MapError> {
        let mut iterations = 0;
        let mut index = SegmentIndex::build(&self.points, START);
        let mut groups = Groups::default();
        index.groups(&self.points, START, &mut groups);
        while self.prune_paths(&mut groups, &index) {
            iterations += 1;
            if iterations > MAX_PRUNE_ITERATIONS {
                return Err(MapError::PruneDidNotConverge);
            }
            index.groups(&self.points, START, &mut groups);
        }
        Ok(())
    }

    /// `PrunePaths`: handles the first group that can be pruned or broken. A group is a list of
    /// the index's runs ([`SegmentIndex::run`]); `segment` is scratch for one run's points.
    fn prune_paths(&mut self, groups: &mut Groups, index: &SegmentIndex) -> bool {
        let mut segment = Vec::with_capacity(32);
        for group in 0..groups.len() {
            let group = groups.group_mut(group);
            unstable_shuffle(group, &mut self.rng);
            if self.prune_all_but_last(group, index, &mut segment) != 0 {
                return true;
            }
            if self.break_parent_child_relationship(group, index, &mut segment) {
                return true;
            }
        }
        false
    }

    fn prune_all_but_last(
        &mut self,
        matches: &[usize],
        index: &SegmentIndex,
        segment: &mut Segment,
    ) -> usize {
        let mut pruned = 0;
        for &run in matches {
            if pruned == matches.len() - 1 {
                return pruned;
            }
            segment.clear();
            segment.extend_from_slice(index.run(run));
            if self.prune_segment(segment) {
                pruned += 1;
            }
        }
        pruned
    }

    fn prune_segment(&mut self, segment: &[Id]) -> bool {
        let mut result = false;
        for i in 0..segment.len() - 1 {
            let id = segment[i];
            if !self.is_in_map(id) {
                return true;
            }
            let point = &self.points[id];
            if point.children.len() > 1
                || point.parents.len() > 1
                || point
                    .parents
                    .iter()
                    .any(|&n| self.points[n].children.len() == 1 && !self.is_removed(n))
            {
                continue;
            }
            let branches_after = segment[i..]
                .iter()
                .any(|&n| self.points[n].children.len() > 1 && self.points[n].parents.len() == 1);
            if branches_after {
                continue;
            }
            if self.points[segment[segment.len() - 1]].parents.len() == 1 {
                return false;
            }
            let leads_to_only_child = point
                .children
                .iter()
                .filter(|c| !segment.contains(c))
                .any(|&c| self.points[c].parents.len() == 1);
            if !leads_to_only_child {
                self.remove_point(id);
                result = true;
            }
        }
        result
    }

    /// `IsInMap`: in the grid, or the start or boss point.
    fn is_in_map(&self, id: Id) -> bool {
        self.grid_cell(id).is_some()
            || matches!(self.kind(id), MapPointType::Ancient | MapPointType::Boss)
    }

    /// `IsRemoved`: the grid cell at the point's coordinate is empty. The start point lives
    /// outside the grid (row 0 is never filled), so the game counts it as removed; the boss
    /// point would index out of range, but is never asked.
    fn is_removed(&self, id: Id) -> bool {
        self.grid_cell(id).is_none()
    }

    fn grid_cell(&self, id: Id) -> Option<Id> {
        let MapCoord { col, row } = self.coord(id);
        self.grid[col][row]
    }

    fn remove_point(&mut self, id: Id) {
        let MapCoord { col, row } = self.coord(id);
        self.grid[col][row] = None;
        self.start_points.retain(|&s| s != id);
        for child in self.points[id].children.clone() {
            self.remove_child(id, child);
        }
        for parent in self.points[id].parents.clone() {
            self.remove_child(parent, id);
        }
    }

    fn break_parent_child_relationship(
        &mut self,
        matches: &[usize],
        index: &SegmentIndex,
        segment: &mut Segment,
    ) -> bool {
        for &run in matches {
            segment.clear();
            segment.extend_from_slice(index.run(run));
            let mut result = false;
            for i in 0..segment.len() - 1 {
                let id = segment[i];
                if self.points[id].children.len() >= 2 {
                    let next = segment[i + 1];
                    if self.points[next].parents.len() != 1 {
                        self.remove_child(id, next);
                        result = true;
                    }
                }
            }
            if result {
                return true;
            }
        }
        false
    }

    // --- post-processing (`MapPostProcessing`) --------------------------------------------

    fn column_is_empty(&self, col: usize) -> bool {
        self.grid[col].iter().all(Option::is_none)
    }

    fn move_to(&mut self, id: Id, col: usize) {
        let MapCoord { col: from, row } = self.coord(id);
        self.grid[from][row] = None;
        self.grid[col][row] = Some(id);
        self.points[id].coord.col = col;
    }

    /// Shifts the whole grid one column when two outer columns on one side are empty and the
    /// other side's are not.
    fn center_grid(&mut self) {
        let left_empty = self.column_is_empty(0) && self.column_is_empty(1);
        let right_empty = self.column_is_empty(COLUMNS - 1) && self.column_is_empty(COLUMNS - 2);
        if left_empty && !right_empty {
            for col in 1..COLUMNS {
                self.shift_column(col, col - 1);
            }
        } else if !left_empty && right_empty {
            for col in (0..COLUMNS - 1).rev() {
                self.shift_column(col, col + 1);
            }
        }
    }

    /// Moves every point of column `from` to the (empty) column `to`, keeping its row.
    fn shift_column(&mut self, from: usize, to: usize) {
        for row in 0..self.rows {
            if let Some(id) = self.grid[from][row].take() {
                self.grid[to][row] = Some(id);
                self.points[id].coord.col = to;
            }
        }
    }

    /// Columns within one step of every parent and child of `id`, ascending.
    fn allowed_columns(&self, id: Id) -> impl Iterator<Item = usize> {
        let point = &self.points[id];
        // Bit `c` is column `c`: the columns within one step of every neighbour.
        let mask = point
            .parents
            .iter()
            .chain(&point.children)
            .fold(u8::MAX, |mask, &n| {
                let col = self.coord(n).col;
                mask & (0b111_u8 << col >> 1)
            });
        (0..COLUMNS).filter(move |&col| mask & (1 << col) != 0)
    }

    /// Smallest column distance from `col` to any other point of the row.
    fn min_gap(&self, col: usize, row: &[Id], id: Id) -> usize {
        row.iter()
            .filter(|&&other| other != id)
            .map(|&other| col.abs_diff(self.coord(other).col))
            .min()
            .unwrap_or(usize::MAX)
    }

    fn spread_adjacent_points(&mut self) {
        for row in 0..self.rows {
            let ids = self.row_points(row);
            loop {
                let mut moved = false;
                for &id in &ids {
                    let col = self.coord(id).col;
                    let mut best = col;
                    let mut best_gap = self.min_gap(col, &ids, id);
                    for candidate in self.allowed_columns(id) {
                        let free = self.grid[candidate][row].is_none_or(|other| other == id);
                        if candidate != col && free {
                            let gap = self.min_gap(candidate, &ids, id);
                            if gap > best_gap {
                                best = candidate;
                                best_gap = gap;
                            }
                        }
                    }
                    if best != col {
                        self.move_to(id, best);
                        moved = true;
                    }
                }
                if !moved {
                    break;
                }
            }
        }
    }

    /// Nudges a point that is a local column extremum on a single line toward its neighbours.
    fn straighten_paths(&mut self) {
        for row in 0..self.rows {
            for col in 0..COLUMNS {
                let Some(id) = self.grid[col][row] else {
                    continue;
                };
                let point = &self.points[id];
                let (&[parent], &[child]) = (point.parents.as_slice(), point.children.as_slice())
                else {
                    continue;
                };
                let here = point.coord.col;
                let (parent_col, child_col) = (self.coord(parent).col, self.coord(child).col);
                let target = if here < child_col && here < parent_col && col < COLUMNS - 1 {
                    Some(col + 1)
                } else if here > child_col && here > parent_col && col > 0 {
                    Some(col - 1)
                } else {
                    None
                };
                if let Some(target) = target
                    && self.grid[target][row].is_none()
                {
                    self.move_to(id, target);
                }
            }
        }
    }

    // --- output ---------------------------------------------------------------------------

    fn neighbors(&self, ids: &[Id]) -> Neighbors {
        let mut neighbors = Neighbors::default();
        for &id in ids {
            neighbors.push(self.coord(id));
        }
        neighbors
    }

    fn finish(&self) -> ActMap {
        let convert = |id: Id| MapPoint {
            coord: self.coord(id),
            kind: self.kind(id),
            parents: self.neighbors(&self.points[id].parents),
            children: self.neighbors(&self.points[id].children),
        };
        // Row by row, each row by column (`ActMap::point` searches this order).
        let points: Vec<MapPoint> = (0..MAX_ROWS)
            .flat_map(|row| self.row_points(row))
            .map(convert)
            .collect();
        ActMap {
            points,
            start: convert(START),
            boss: convert(BOSS),
            second_boss: self.has_second_boss.then(|| convert(SECOND_BOSS)),
            spoils: self.kind == Kind::Spoils,
        }
    }
}

/// `SpoilsActMap.BuildCenteredPriorityList`: towards the centre, straight, away from it, then
/// whatever of -1 and 1 is left.
fn centred_offsets(col: usize, center: usize) -> [i8; 3] {
    match col.cmp(&center) {
        std::cmp::Ordering::Less => [1, 0, -1],
        std::cmp::Ordering::Equal => [0, -1, 1],
        std::cmp::Ordering::Greater => [-1, 0, 1],
    }
}

/// `OverlappingSegment`: two segments of at least 3 points share an interior point at the same index.
fn overlapping(a: &[Id], b: &[Id]) -> bool {
    a.len() >= 3 && b.len() >= 3 && (1..=a.len() - 2).any(|i| a[i] == b[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(seed: &str) -> ActMap {
        ActMap::generate_act1(&RunRngSet::new(seed), 0).expect("map generates")
    }

    #[test]
    fn hive_maps_are_one_row_shorter() {
        for n in 0..20 {
            let m = ActMap::generate(&RunRngSet::new(&format!("RL1S{n}")), 1, 0).expect("map");
            assert_eq!(m.boss().coord, MapCoord { col: 3, row: 15 });
            assert!(m.points().iter().all(|p| (1..=14).contains(&p.coord.row)));
            for p in m.points() {
                match p.coord.row {
                    14 => assert_eq!(p.kind, MapPointType::RestSite),
                    8 => assert_eq!(p.kind, MapPointType::Treasure),
                    _ => {}
                }
            }
        }
    }

    /// `Glory`: 13 rooms (boss on row 14, treasure on row 7, rests on row 13).
    #[test]
    fn glory_maps_have_thirteen_rooms() {
        for n in 0..20 {
            let m = ActMap::generate(&RunRngSet::new(&format!("RL1S{n}")), 2, 0).expect("map");
            assert_eq!(m.boss().coord, MapCoord { col: 3, row: 14 });
            assert!(m.second_boss().is_none());
            assert!(m.points().iter().all(|p| (1..=13).contains(&p.coord.row)));
            for p in m.points() {
                match p.coord.row {
                    13 => assert_eq!(p.kind, MapPointType::RestSite),
                    7 => assert_eq!(p.kind, MapPointType::Treasure),
                    _ => {}
                }
            }
        }
    }

    /// `Glory.GetMapPointTypes`: rests are one `NextInt(5, 7)`, then the unknowns' gaussian.
    #[test]
    fn glory_rest_count_is_next_int_five_to_seven() {
        let mut drawn_counts = [false; 2];
        for n in 0..50 {
            let seed = RunRngSet::new(&format!("RL1S{n}")).seed();
            let mut expected = Rng::named(seed, "act_3_map");
            let rests = expected.next_int_range(5, 7);
            let mut rng = Rng::named(seed, "act_3_map");
            let drawn = act_shape(2).rests.draw(&mut rng);
            assert_eq!(drawn, usize::try_from(rests).expect("5 or 6"));
            assert_eq!(rng, expected, "a single draw");
            drawn_counts[drawn - 5] = true;
        }
        assert_eq!(drawn_counts, [true, true]);
        // Hive's draw is the gaussian, a different number of raw draws.
        let mut rng = Rng::named(RunRngSet::new("RL1S0").seed(), "act_2_map");
        let mut expected = rng.clone();
        act_shape(1).rests.draw(&mut rng);
        expected.next_gaussian_int(6, 1, 6, 7);
        assert_eq!(rng, expected);
    }

    /// `StandardActMap(hasSecondBoss)`: from `DoubleBoss` (A10) the last act's boss has a single
    /// child, the second boss at `(3, rows + 1)`, typed Boss; the other acts and A9 have none.
    #[test]
    fn double_boss_adds_a_second_boss_point_to_the_last_act() {
        for n in 0..20 {
            let rngs = RunRngSet::new(&format!("RL1S{n}"));
            let a10 = ActMap::generate(&rngs, 2, 10).expect("map");
            let second = a10.second_boss().expect("a second boss");
            assert_eq!(second.coord, MapCoord { col: 3, row: 15 });
            assert_eq!(second.kind, MapPointType::Boss);
            assert_eq!(second.parents, vec![a10.boss().coord]);
            assert_eq!(second.children, Vec::new());
            assert_eq!(a10.boss().children, vec![second.coord]);
            assert_eq!(a10.point(second.coord), Some(second));
            // Only the boss's edge differs from the map without it (the A9 elite count is the same).
            let a9 = ActMap::generate(&rngs, 2, 9).expect("map");
            assert!(a9.second_boss().is_none() && a9.boss().children.is_empty());
            assert_eq!(a9.points(), a10.points());
            for act in 0..2 {
                assert!(
                    ActMap::generate(&rngs, act, 10)
                        .expect("map")
                        .second_boss()
                        .is_none()
                );
            }
        }
    }

    #[test]
    fn spoils_maps_pass_through_one_treasure() {
        for n in 0..50 {
            let m =
                ActMap::generate_spoils(&RunRngSet::new(&format!("RL1S{n}")), 1, 0).expect("map");
            let treasures: Vec<_> = m
                .points()
                .iter()
                .filter(|p| p.kind == MapPointType::Treasure)
                .collect();
            assert_eq!(treasures.len(), 1);
            assert_eq!(treasures[0].coord, MapCoord { col: 3, row: 8 });
            assert_eq!(m.points().iter().filter(|p| p.coord.row == 8).count(), 1);
            assert!(
                m.point(MapCoord { col: 3, row: 8 })
                    .is_some_and(|p| !p.children.is_empty())
            );
        }
    }

    /// FNV-1a over the `Debug` form of every map (or error) and the map stream's final state, for
    /// RL1S0..RL1S<seeds> × acts 0-2 × A0/A10, plus the Spoils map at A0.
    fn map_digest(seeds: usize) -> u64 {
        let mut hash = 0xCBF2_9CE4_8422_2325_u64;
        let mut fold = |text: String| {
            for byte in text.bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3);
            }
        };
        for n in 0..seeds {
            let rngs = RunRngSet::new(&format!("RL1S{n}"));
            for act in 0..=LAST_ACT {
                for ascension in [0, 10] {
                    let shape = act_shape(act);
                    let rng = Rng::named(rngs.seed(), &format!("act_{}_map", act + 1));
                    let mut builder =
                        Builder::new(rng, &shape, elite_count(ascension), Kind::Standard);
                    if act == LAST_ACT && AscensionLevel::DoubleBoss.is_active(ascension) {
                        builder.add_second_boss();
                    }
                    fold(format!("{:?}", builder.build_traced()));
                }
                let rng = Rng::named(rngs.seed(), "spoils_map");
                let builder = Builder::new(rng, &act_shape(act), elite_count(0), Kind::Spoils);
                fold(format!("{:?}", builder.build_traced()));
            }
        }
        hash
    }

    /// Pins generation (maps, errors and RNG draws) to the output recorded before PF2 replaced the
    /// from-scratch segment search with the incremental index.
    #[test]
    fn map_digest_is_unchanged() {
        assert_eq!(map_digest(200), 0xA926_ACEB_5113_E5DE);
    }

    /// [`map_digest_is_unchanged`] over 2,000 seeds (about 10 s in release, a minute in debug):
    /// `cargo test --release -p sts2-core --lib -- --ignored map_digest`.
    #[test]
    #[ignore = "slow; run in release"]
    fn map_digest_is_unchanged_2000_seeds() {
        assert_eq!(map_digest(2000), 0x1572_912E_CBFE_5DC0);
    }

    /// The from-scratch `FindMatchingSegments` that [`SegmentIndex`] replaced: every path from the
    /// start, every segment of each, keyed by `GenerateSegmentKey`'s string.
    fn find_matching_segments(builder: &Builder) -> Vec<Vec<Segment>> {
        fn paths(builder: &Builder, id: Id) -> Vec<Segment> {
            if builder.kind(id) == MapPointType::Boss {
                return vec![vec![id]];
            }
            let mut out = Vec::new();
            for &child in &builder.points[id].children {
                for tail in paths(builder, child) {
                    out.push([vec![id], tail].concat());
                }
            }
            out
        }
        let mut groups: std::collections::BTreeMap<String, Vec<Segment>> =
            std::collections::BTreeMap::new();
        for path in paths(builder, START) {
            for i in 0..path.len() - 1 {
                let start = &builder.points[path[i]];
                if start.children.len() <= 1 && start.coord.row != 0 {
                    continue;
                }
                for j in 2..path.len() - i {
                    if builder.points[path[i + j]].parents.len() < 2 {
                        continue;
                    }
                    let segment = path[i..=i + j].to_vec();
                    let first = builder.coord(segment[0]);
                    let last = builder.coord(segment[j]);
                    let types: Vec<String> = segment
                        .iter()
                        .map(|&id| builder.kind(id).index().to_string())
                        .collect();
                    let ends = if first.row == 0 {
                        format!("{}-{},{}-", first.row, last.col, last.row)
                    } else {
                        format!("{},{}-{},{}-", first.col, first.row, last.col, last.row)
                    };
                    let group = groups.entry(ends + &types.join(",")).or_default();
                    if !group.iter().any(|other| overlapping(other, &segment)) {
                        group.push(segment);
                    }
                }
            }
        }
        groups.into_values().filter(|g| g.len() > 1).collect()
    }

    /// The index's groups equal the from-scratch search's after every prune of every pass.
    #[test]
    fn segment_index_matches_the_from_scratch_search() {
        let mut checked = 0;
        for n in 0..40 {
            let rngs = RunRngSet::new(&format!("RL1S{n}"));
            for act in 0..=LAST_ACT {
                let rng = Rng::named(rngs.seed(), &format!("act_{}_map", act + 1));
                let mut b = Builder::new(rng, &act_shape(act), ELITES, Kind::Standard);
                b.generate_paths().expect("paths");
                b.assign_point_types();
                for _ in 0..3 {
                    let mut index = SegmentIndex::build(&b.points, START);
                    let mut groups = Groups::default();
                    loop {
                        index.groups(&b.points, START, &mut groups);
                        let segments: Vec<Vec<Segment>> = (0..groups.len())
                            .map(|g| {
                                let group = groups.group_mut(g);
                                group.iter().map(|&run| index.run(run).to_vec()).collect()
                            })
                            .collect();
                        assert_eq!(segments, find_matching_segments(&b), "RL1S{n} act {act}");
                        checked += 1;
                        if !b.prune_paths(&mut groups, &index) {
                            break;
                        }
                    }
                    if !b.repair_pruned_point_types() {
                        break;
                    }
                }
            }
        }
        assert!(checked > 400, "{checked} searches");
    }

    #[test]
    fn generation_is_deterministic() {
        assert_eq!(map("RL1S0"), map("RL1S0"));
        assert_ne!(map("RL1S0"), map("RL1S1"));
    }

    #[test]
    fn maps_are_well_formed() {
        for n in 0..50 {
            let m = map(&format!("RL1S{n}"));
            assert_eq!(m.start().coord, MapCoord { col: 3, row: 0 });
            assert_eq!(m.boss().coord, MapCoord { col: 3, row: 16 });
            assert_eq!(m.start().kind, MapPointType::Ancient);
            assert_eq!(m.boss().kind, MapPointType::Boss);
            for p in m.points() {
                assert!((1..=15).contains(&p.coord.row) && p.coord.col < COLUMNS);
                if p.coord.row == 15 {
                    assert_eq!(p.kind, MapPointType::RestSite);
                }
                if p.coord.row == 9 {
                    assert_eq!(p.kind, MapPointType::Treasure);
                }
                if p.coord.row == 1 {
                    assert_eq!(p.kind, MapPointType::Monster);
                }
                // Edges are consistent in both directions and go down one row.
                assert!(!p.children.is_empty(), "{:?} has no children", p.coord);
                for c in &p.children {
                    let child = m.point(c).expect("child exists");
                    assert_eq!(c.row, p.coord.row + 1);
                    assert!(child.parents.contains(&p.coord));
                }
            }
        }
    }
}
