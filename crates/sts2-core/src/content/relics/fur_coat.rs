//! `FurCoat` (`Models.Relics/FurCoat.cs`): Ancient (Nonupeipe); marks 8 random Monster/Elite
//! points of the act's map (`AddMarkedRooms`: `AfterObtained` on the current map,
//! `ModifyGeneratedMapLate` on a regenerated one, drawn on `new Rng(Owner, Id, 0)` =
//! `Rng::named(run seed, "FUR_COAT")`). In a fight at a marked point every enemy has 1 HP
//! (`BeforeCombatStart`, and each enemy added later).
//!
//! Map quest markers (`MapPoint.AddQuest`, `IsAffectedByFurCoat` of the history entry) are not
//! modelled: the coordinates are the relic's own fields.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;
use crate::rng::Rng;
use crate::run::map::{ActMap, MapCoord, MapError, MapPointType};
use crate::run::{RunCtx, RunError, RunState};
use crate::shuffle::unstable_shuffle;

/// `DynamicVar("Combats", 8m)`.
const COMBATS: usize = 8;

pub struct FurCoat;

/// `FurCoat`'s fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `FurCoatActIndex`, -1 until obtained.
    pub fur_coat_act_index: i32,
    /// `FurCoatCoordCols`: the first `len` slots.
    pub cols: [u8; COMBATS],
    /// `FurCoatCoordRows`: the first `len` slots.
    pub rows: [u8; COMBATS],
    /// The length of both arrays.
    pub len: u8,
    /// `FurCoatCoordsSet`.
    pub coords_set: bool,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            fur_coat_act_index: -1,
            cols: [0; COMBATS],
            rows: [0; COMBATS],
            len: 0,
            coords_set: false,
        }
    }
}

impl Data {
    /// The number of marked points (`FurCoatCoordCols.Length`).
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    /// Whether no point is marked.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// `GetMarkedCoords()`, null until set.
    fn marked(&self) -> Option<impl Iterator<Item = MapCoord>> {
        self.coords_set.then(|| {
            (0..self.len()).map(|i| MapCoord {
                col: usize::from(self.cols[i]),
                row: usize::from(self.rows[i]),
            })
        })
    }

    /// `GetMarkedCoords() != null && Contains(coord)`.
    fn marks(&self, (col, row): (i32, i32)) -> bool {
        self.marked().is_some_and(|mut coords| {
            coords.any(|c| i32::try_from(c.col) == Ok(col) && i32::try_from(c.row) == Ok(row))
        })
    }
}

fn data(s: &RunState, me: usize) -> Data {
    let RelicData::FurCoat(data) = s.relics[me].data else {
        unreachable!("a FurCoat has FurCoat data");
    };
    data
}

/// `AddMarkedRooms(map)`: the marked points are drawn again unless they are all Monster or Elite
/// points of this map.
fn add_marked_rooms(s: &mut RunState, me: usize, map: &ActMap) {
    let mut data = data(s, me);
    if i32::try_from(s.act_index).ok() != Some(data.fur_coat_act_index) {
        return;
    }
    let is_combat_point = |c: MapCoord| {
        map.point(c)
            .is_some_and(|p| matches!(p.kind, MapPointType::Monster | MapPointType::Elite))
    };
    if data
        .marked()
        .is_some_and(|mut coords| coords.all(is_combat_point))
    {
        return;
    }
    let mut rng = Rng::named(s.rng.seed(), "FUR_COAT");
    // `GetAllMapPoints`: column by column, each from row 0 up.
    let mut points: Vec<MapCoord> = map
        .points()
        .iter()
        .filter(|p| matches!(p.kind, MapPointType::Monster | MapPointType::Elite))
        .map(|p| p.coord)
        .collect();
    points.sort_unstable();
    unstable_shuffle(&mut points, &mut rng);
    points.truncate(COMBATS);
    data.len = u8::try_from(points.len()).expect("at most 8 points");
    for (i, coord) in points.iter().enumerate() {
        data.cols[i] = u8::try_from(coord.col).expect("a map column fits u8");
        data.rows[i] = u8::try_from(coord.row).expect("a map row fits u8");
    }
    data.coords_set = true;
    s.relics[me].data = RelicData::FurCoat(data);
}

/// `CreatureCmd.SetCurrentHp(creature, 1)`.
fn set_hp_to_1(eng: &mut Engine<'_>, creature: CreatureId) -> Flow {
    let old = eng.s.creature(creature).hp;
    let c = eng.s.creature_mut(creature);
    c.hp = c.max_hp.min(1);
    let new = c.hp;
    eng.s.recount_enemies();
    if new != old {
        eng.after_current_hp_changed(creature, Decimal::from(new - old))?;
    }
    if eng.s.creature(creature).is_dead() {
        eng.kill(&[creature])?;
    }
    Ok(())
}

/// Whether the fight is at one of the relic's marked points.
fn at_marked_point(s: &CombatState, me: RelicId) -> bool {
    let RelicData::FurCoat(data) = s.relic(me).data else {
        unreachable!("a FurCoat has FurCoat data");
    };
    s.map_coord.is_some_and(|c| data.marks(c))
}

impl RelicModel for FurCoat {
    fn name(&self) -> &'static str {
        "Fur Coat"
    }

    fn init_data(&self) -> RelicData {
        RelicData::FurCoat(Data::default())
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let mut data = data(s, me);
        data.fur_coat_act_index = i32::try_from(s.act_index).expect("an act index fits i32");
        s.relics[me].data = RelicData::FurCoat(data);
        let map = s.map.clone();
        add_marked_rooms(s, me, &map);
        Ok(())
    }

    fn modify_generated_map_late(
        &self,
        s: &mut RunState,
        me: usize,
        map: &mut ActMap,
        _act_index: usize,
    ) -> Result<(), MapError> {
        add_marked_rooms(s, me, map);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for FurCoat {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        if !at_marked_point(eng.s, me) {
            return Ok(());
        }
        for enemy in eng.hooked().hittable_enemies() {
            set_hp_to_1(eng, enemy)?;
        }
        Ok(())
    }

    fn after_creature_added_to_combat(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        creature: CreatureId,
    ) -> Flow {
        if eng.s.creature(creature).side == Side::Enemy && at_marked_point(eng.s, me) {
            set_hp_to_1(eng, creature)?;
        }
        Ok(())
    }
}
