//! The combat gym (RLN43): fight starts rebuilt from strong human runs.
//!
//! A [`HumanStart`] is one combat room of a recorded run as it was entered: the act, the act
//! model, the encounter, the total floor, HP, max HP, gold, the deck with upgrades, the relics,
//! the belt's size and potions. [`HumanStart::build`] turns it into a [`Run`] standing in that
//! fight:
//! - a fresh Necrobinder A10 run on the start's seed (`GYM<seed>`), its Neow event taken by its
//!   first options until the map, then the start's act entered (`Run::debug_enter_act`);
//! - on the map (`Run::loadout_state`): the act's model, its next encounter of the room's type
//!   set to the start's (`RoomSet`), the deck, relics (fresh: [`RunRelic::new`]), belt, HP, max
//!   HP and gold set, the history padded to the start's floor, and the player put on a parent of
//!   a point of the room's type (the boss point, or a point drawn from the seed);
//! - the move into that point: the fight is set up by the run as any other (the encounter's
//!   monsters and their AI on the run's fresh streams).
//!
//! What the record lacks is approximated (documented in `docs/research/rln43-combat-gym.md`):
//! relic counters start fresh, cards have no enchantment and no counter, the belt's order is
//! the order the potions were gained, and the run's other state (grab bags, odds, visited
//! events) is the fresh run's.
//!
//! A [`Gym`] holds the starts by [`GymCell`] and hands out [`GymTicket`]s: drawn by cell weight
//! (uniform within a cell), or in record order for a benchmark ([`GymConfig::sequential`]). A
//! ticket is built in the env's own thread when it resets. Optional variants
//! ([`GymConfig::variants`]): HP drawn in 40-100% of max HP, and one card upgraded or downgraded.
//! A hallway start's HP is capped at [`GymConfig::hall_hp_pct`] percent of max HP.

use std::sync::Arc;

use serde_json::Value;
use sts2_core::Character;
use sts2_core::combat::Encounter;
use sts2_core::content::cards::CardKind;
use sts2_core::content::potions::PotionKind;
use sts2_core::content::relics::RelicKind;
use sts2_core::run::rooms::{ACTS_BY_INDEX, ActInfo, EncounterInfo, EncounterRoom};
use sts2_core::run::{
    Act1, MapCoord, MapPointHistoryEntry, MapPointType, RoomType, Run, RunAction, RunCard,
    RunDecision, RunRelic, UnlockState,
};

use crate::seeds::SplitMix;

/// Cards whose saved counter decides what they are; a record can't rebuild them.
const UNRECORDED_COUNTER_CARDS: [&str; 1] = ["MAD_SCIENCE"];

/// The number of gym cells.
pub const GYM_CELLS: usize = 9;

/// The gym cells' names, by [`GymCell::index`].
pub const GYM_CELL_NAMES: [&str; GYM_CELLS] = [
    "GYM_ELITE_ACT1",
    "GYM_ELITE_ACT2",
    "GYM_ELITE_ACT3",
    "GYM_BOSS_ACT1",
    "GYM_BOSS_ACT2",
    "GYM_BOSS_ACT3",
    "GYM_HALL_ACT1",
    "GYM_HALL_ACT2",
    "GYM_HALL_ACT3",
];

/// A gym start's room: elite, boss or hallway (a monster room) of act index `act` (0 to 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GymCell {
    pub room: EncounterRoom,
    pub act: u8,
}

impl GymCell {
    /// 0-2 the elites, 3-5 the bosses, 6-8 the hallways of acts 1-3 ([`GYM_CELL_NAMES`]).
    #[must_use]
    pub const fn index(self) -> usize {
        let base = match self.room {
            EncounterRoom::Elite => 0,
            EncounterRoom::Boss => 3,
            EncounterRoom::Monster => 6,
        };
        base + self.act as usize
    }
}

/// One combat room of a human run, as it was entered (see the module docs).
#[derive(Clone, Debug)]
pub struct HumanStart {
    /// The act index, 0 to 2.
    pub act: u8,
    pub act_info: &'static ActInfo,
    pub encounter: &'static EncounterInfo,
    /// The total floor of the fight's point (Neow is 1).
    pub floor: usize,
    pub hp: i32,
    pub max_hp: i32,
    pub gold: i32,
    pub deck: Vec<RunCard>,
    pub relics: Vec<RelicKind>,
    pub potion_slots: usize,
    pub potions: Vec<&'static str>,
}

/// A variant of a start ([`GymConfig::variants`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Variant {
    /// HP as permille of max HP (0: the record's HP).
    pub hp_permille: u32,
    /// A deck index to upgrade (`up`) or downgrade, if any.
    pub card: Option<(usize, bool)>,
}

fn text<'a>(v: &'a Value, name: &str) -> Result<&'a str, String> {
    v.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("field {name}"))
}

fn int<T: TryFrom<i64>>(v: &Value, name: &str) -> Result<T, String> {
    v.get(name)
        .and_then(Value::as_i64)
        .and_then(|n| T::try_from(n).ok())
        .ok_or_else(|| format!("field {name}"))
}

fn array<'a>(v: &'a Value, name: &str) -> Result<&'a [Value], String> {
    v.get(name)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("field {name}"))
}

impl HumanStart {
    /// A start from one JSON line: `act`, `act_id`, `encounter`, `floor`, `hp`, `max_hp`,
    /// `gold`, `deck` [[card id, upgrade]], `relics` [id], `potion_slots`, `potions` [id].
    ///
    /// # Errors
    /// A message starting with the kind of what is wrong (`field`, `act`, `encounter`, `card`,
    /// `relic`, `potion`) for a malformed line or content the engine does not port.
    pub fn from_json(line: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("field json: {e}"))?;
        let act: u8 = int(&v, "act")?;
        let acts = ACTS_BY_INDEX
            .get(usize::from(act))
            .ok_or_else(|| format!("act {act}"))?;
        let act_id = text(&v, "act_id")?;
        let act_info = *acts
            .iter()
            .find(|a| a.id == act_id)
            .ok_or_else(|| format!("act {act_id} at index {act}"))?;
        let id = text(&v, "encounter")?;
        let encounter = act_info
            .encounters
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| format!("encounter {id} not in {act_id}"))?;
        if Encounter::from_entry(id).is_none() {
            return Err(format!("encounter {id}"));
        }
        let deck = array(&v, "deck")?
            .iter()
            .map(|c| {
                let id = c.get(0).and_then(Value::as_str).ok_or("field deck")?;
                let up = c
                    .get(1)
                    .and_then(Value::as_u64)
                    .and_then(|u| u8::try_from(u).ok())
                    .ok_or("field deck")?;
                // `MadScience`'s type and rider live in its counter, which the record lacks.
                if UNRECORDED_COUNTER_CARDS.contains(&id) {
                    return Err(format!("card {id} (counter not recorded)"));
                }
                let kind = CardKind::from_id(id).ok_or_else(|| format!("card {id}"))?;
                let mut card = RunCard::new(kind.entry());
                card.upgrade_level = up;
                if card.to_combat().is_none() {
                    return Err(format!("card {id}"));
                }
                Ok(card)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let relics = array(&v, "relics")?
            .iter()
            .map(|r| {
                let id = r.as_str().ok_or("field relics")?;
                RelicKind::from_id(id).ok_or_else(|| format!("relic {id}"))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let potions = array(&v, "potions")?
            .iter()
            .map(|p| {
                let id = p.as_str().ok_or("field potions")?;
                PotionKind::from_id(id)
                    .map(PotionKind::id)
                    .ok_or_else(|| format!("potion {id}"))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let start = Self {
            act,
            act_info,
            encounter,
            floor: int(&v, "floor")?,
            hp: int(&v, "hp")?,
            max_hp: int(&v, "max_hp")?,
            gold: int(&v, "gold")?,
            deck,
            relics,
            potion_slots: int(&v, "potion_slots")?,
            potions,
        };
        if start.max_hp < 1 || start.hp < 1 || start.floor < 2 {
            return Err("field hp or floor".into());
        }
        Ok(start)
    }

    /// The start's cell.
    #[must_use]
    pub const fn cell(&self) -> GymCell {
        GymCell {
            room: self.encounter.room,
            act: self.act,
        }
    }

    /// The deck with the variant's card upgraded or downgraded.
    fn deck_with(&self, variant: Variant) -> Vec<RunCard> {
        let mut deck = self.deck.clone();
        if let Some((i, up)) = variant.card
            && let Some(card) = deck.get_mut(i)
        {
            if up {
                card.upgrade();
            } else if card.upgrade_level > 0 {
                card.downgrade();
            }
        }
        deck
    }

    /// The entry HP: the record's, or the variant's share of max HP, then the cap (% of max HP).
    fn entry_hp(&self, variant: Variant, hp_cap_pct: u32) -> i32 {
        let mut hp = self.hp.min(self.max_hp);
        if variant.hp_permille > 0 {
            let p = i64::from(variant.hp_permille);
            hp = i32::try_from((i64::from(self.max_hp) * p / 1000).max(1)).unwrap_or(hp);
        }
        if hp_cap_pct > 0 {
            let cap = i64::from(self.max_hp) * i64::from(hp_cap_pct) / 100;
            hp = hp.min(i32::try_from(cap.max(1)).unwrap_or(hp));
        }
        hp
    }

    /// The run standing in the start's fight, on seed `seed` (see the module docs), with
    /// `variant` applied and HP capped at `hp_cap_pct` percent of max HP (0: no cap).
    ///
    /// # Errors
    /// If the run cannot be set up (a message; the fight's own set-up errors included).
    pub fn build(&self, seed: u64, variant: Variant, hp_cap_pct: u32) -> Result<Run, String> {
        let mut run = Run::new(
            Character::Necrobinder,
            &format!("GYM{seed}"),
            10,
            UnlockState::all(),
            Act1::Random,
        )
        .map_err(|e| format!("run {e:?}"))?;
        // Neow: its first options until the map (what they give is replaced below).
        for _ in 0..64 {
            if matches!(run.pending_decision(), RunDecision::Map(_)) {
                break;
            }
            let Some(action) = run.legal_actions().into_iter().next() else {
                return Err("neow: no option".into());
            };
            run.apply(action).map_err(|e| format!("neow {e:?}"))?;
        }
        if !matches!(run.pending_decision(), RunDecision::Map(_)) {
            return Err("neow: no map".into());
        }
        let act = usize::from(self.act);
        if act > 0 {
            run.debug_enter_act(act)
                .map_err(|e| format!("enter act {e:?}"))?;
        }
        let mut rng = SplitMix::new(seed ^ 0x4759_4d5f_5354_4152);
        let s = run.loadout_state().map_err(|e| format!("loadout {e:?}"))?;
        let rooms = &mut s.acts[act];
        rooms.act = self.act_info;
        let kind = match self.encounter.room {
            EncounterRoom::Monster => {
                rooms.normal_encounters = vec![self.encounter];
                rooms.normal_encounters_visited = 0;
                MapPointType::Monster
            }
            EncounterRoom::Elite => {
                rooms.elite_encounters = vec![self.encounter];
                rooms.elite_encounters_visited = 0;
                MapPointType::Elite
            }
            EncounterRoom::Boss => {
                rooms.boss = self.encounter;
                rooms.second_boss = None;
                rooms.boss_encounters_visited = 0;
                MapPointType::Boss
            }
        };
        s.deck = self.deck_with(variant);
        s.relics = self.relics.iter().map(|&k| RunRelic::new(k)).collect();
        let slots = self.potion_slots.max(self.potions.len());
        s.potions = (0..slots).map(|i| self.potions.get(i).copied()).collect();
        s.max_hp = self.max_hp;
        s.hp = self.entry_hp(variant, hp_cap_pct);
        s.gold = self.gold;
        // The target point and a parent to stand on.
        let target = if kind == MapPointType::Boss {
            *s.map.boss()
        } else {
            let points: Vec<_> = s
                .map
                .points()
                .iter()
                .filter(|p| p.kind == kind && p.parents.iter().next().is_some())
                .copied()
                .collect();
            if points.is_empty() {
                return Err(format!("map: no {kind:?} point"));
            }
            points[rng.below(points.len())]
        };
        let parents: Vec<MapCoord> = target.parents.iter().collect();
        let Some(&parent) = parents.get(rng.below(parents.len().max(1))) else {
            return Err("map: no parent".into());
        };
        s.visited = vec![parent];
        s.act_floor = parent.row + 1;
        // The history: one entry per point before the fight's (`TotalFloor` = floor once in).
        let pad = MapPointHistoryEntry {
            point_type: MapPointType::Monster,
            room_type: Some(RoomType::Monster),
            model: None,
        };
        s.history.truncate(self.floor - 1);
        s.history.resize(self.floor - 1, pad);
        let choice = RunAction::ChooseNode(target.coord);
        if !run.accepts(&choice) {
            return Err("map: move refused".into());
        }
        run.apply(choice).map_err(|e| format!("fight {e:?}"))?;
        if run.combat().is_none() {
            return Err("fight: not in combat".into());
        }
        Ok(run)
    }
}

/// The start record ([`HumanStart::from_json`]'s line) of the fight `run` stands in, if any.
///
/// NAZ1 uses it for fight starts from the bot's own runs. It carries what a human record
/// carries: HP, max HP and gold as the run holds them (at a fight's first decision: as entered,
/// unless a start-of-fight effect changed the run's HP), the deck's ids and upgrade levels (no
/// enchantments or counters), the relic ids (no counters) and the belt's potions in slot order.
#[must_use]
pub fn start_record(run: &Run) -> Option<String> {
    let s = run.state();
    let sts2_core::run::Room::Combat { encounter, .. } = s.room else {
        return None;
    };
    let rooms = s.acts.get(s.act_index)?;
    let deck: Vec<Value> = s
        .deck
        .iter()
        .map(|c| serde_json::json!([c.id, c.upgrade_level]))
        .collect();
    let relics: Vec<&str> = s.relics.iter().map(|r| r.id()).collect();
    let potions: Vec<&str> = s.potions.iter().flatten().copied().collect();
    Some(
        serde_json::json!({
            "act": s.act_index,
            "act_id": rooms.act.id,
            "encounter": encounter,
            "floor": s.total_floor(),
            "hp": s.hp,
            "max_hp": s.max_hp,
            "gold": s.gold,
            "deck": deck,
            "relics": relics,
            "potion_slots": s.potions.len(),
            "potions": potions,
        })
        .to_string(),
    )
}

/// The gym's settings ([`crate::RunEnvs::set_gym`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GymConfig {
    /// Envs `first..first + slots` start a gym fight when their episode ends.
    pub first: usize,
    pub slots: usize,
    /// Relative weights of the cells for one start, by [`GymCell::index`].
    pub weights: [u32; GYM_CELLS],
    /// HP in 40-100% of max HP and one card up or down a level, drawn per start.
    pub variants: bool,
    /// A hallway start's HP cap, percent of max HP (0: none).
    pub hall_hp_pct: u32,
    /// The records in order, each once, on seeds `seed + k` (a benchmark); the weights are
    /// ignored and the variants off.
    pub sequential: bool,
    /// The seed of the draws (read when the gym is made or reconfigured with another seed).
    pub seed: u64,
}

/// Counts per cell, by [`GymCell::index`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GymStats {
    /// Starts loaded.
    pub loaded: [u64; GYM_CELLS],
    /// Draws of the cell.
    pub wanted: [u64; GYM_CELLS],
    /// Tickets handed out.
    pub started: [u64; GYM_CELLS],
    /// Draws made while the cell had weight but no start (its weight went to the others).
    pub empty: [u64; GYM_CELLS],
}

/// A start to build in the env's thread.
#[derive(Clone, Debug)]
pub struct GymTicket {
    pub start: Arc<HumanStart>,
    /// The record's index in the loaded order.
    pub index: usize,
    pub seed: u64,
    pub variant: Variant,
    pub hp_cap_pct: u32,
}

impl GymTicket {
    /// The cell code of the episode: 7 + the cell's index (practice cells are 1-6).
    ///
    /// # Panics
    /// Never: there are 9 cells.
    #[must_use]
    pub fn code(&self) -> u8 {
        u8::try_from(7 + self.start.cell().index()).expect("few cells")
    }

    /// The run in the fight.
    ///
    /// # Errors
    /// As [`HumanStart::build`].
    pub fn build(&self) -> Result<Run, String> {
        self.start.build(self.seed, self.variant, self.hp_cap_pct)
    }
}

/// The loaded starts and the draws.
#[derive(Debug)]
pub struct Gym {
    starts: Vec<Arc<HumanStart>>,
    by_cell: [Vec<usize>; GYM_CELLS],
    config: GymConfig,
    rng: SplitMix,
    next: usize,
    stats: GymStats,
}

impl Gym {
    /// A gym over `starts`.
    #[must_use]
    pub fn new(starts: Vec<HumanStart>, config: GymConfig) -> Self {
        let mut by_cell: [Vec<usize>; GYM_CELLS] = std::array::from_fn(|_| Vec::new());
        let mut counts = GymStats::default();
        for (i, start) in starts.iter().enumerate() {
            let c = start.cell().index();
            by_cell[c].push(i);
            counts.loaded[c] += 1;
        }
        Self {
            starts: starts.into_iter().map(Arc::new).collect(),
            by_cell,
            config,
            rng: SplitMix::new(config.seed ^ 0x4759_4d44_5241_5753),
            next: 0,
            stats: counts,
        }
    }

    /// Parses JSON lines (blank lines skipped).
    ///
    /// # Errors
    /// The first bad line's number and message.
    pub fn parse(text: &str) -> Result<Vec<HumanStart>, String> {
        text.lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
            .map(|(i, l)| HumanStart::from_json(l).map_err(|e| format!("line {}: {e}", i + 1)))
            .collect()
    }

    /// New settings; the starts and the counts stay, the draws restart on a new seed, and the
    /// sequential order restarts when it is turned on.
    pub const fn configure(&mut self, config: GymConfig) {
        if config.seed != self.config.seed {
            self.rng = SplitMix::new(config.seed ^ 0x4759_4d44_5241_5753);
        }
        if config.sequential && !self.config.sequential {
            self.next = 0;
        }
        self.config = config;
    }

    #[must_use]
    pub const fn config(&self) -> &GymConfig {
        &self.config
    }

    #[must_use]
    pub const fn stats(&self) -> &GymStats {
        &self.stats
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.starts.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }

    /// Whether env `i` is a gym slot.
    #[must_use]
    pub const fn covers(&self, i: usize) -> bool {
        i >= self.config.first && i < self.config.first + self.config.slots
    }

    /// A cell by weight among the cells that hold records; a weighted empty cell's weight goes
    /// to the others (counted in `empty`), so a gym slot never falls back to a normal run.
    fn draw_cell(&mut self) -> Option<usize> {
        let mut weights = [0_u64; GYM_CELLS];
        for (i, &w) in self.config.weights.iter().enumerate() {
            if w == 0 {
                continue;
            }
            if self.by_cell[i].is_empty() {
                self.stats.empty[i] += 1;
            } else {
                weights[i] = u64::from(w);
            }
        }
        let total: u64 = weights.iter().sum();
        if total == 0 {
            return None;
        }
        let mut x = self.rng.next() % total;
        for (i, &w) in weights.iter().enumerate() {
            if x < w {
                return Some(i);
            }
            x -= w;
        }
        None
    }

    /// The next start, or `None` (no weighted cell holds records, or the sequential order done).
    pub fn draw(&mut self) -> Option<GymTicket> {
        let c = self.config;
        let (index, seed, variant) = if c.sequential {
            let k = self.next;
            if k >= self.starts.len() {
                return None;
            }
            self.next += 1;
            (k, c.seed.wrapping_add(k as u64), Variant::default())
        } else {
            let cell = self.draw_cell()?;
            self.stats.wanted[cell] += 1;
            let list = &self.by_cell[cell];
            let index = list[self.rng.below(list.len())];
            let seed = self.rng.next() >> 16;
            let mut variant = Variant::default();
            if c.variants {
                variant.hp_permille = 400 + u32::try_from(self.rng.next() % 601).unwrap_or(0);
                let n = self.starts[index].deck.len();
                if n > 0 {
                    variant.card = Some((self.rng.below(n), self.rng.next() & 1 == 0));
                }
            }
            (index, seed, variant)
        };
        let start = Arc::clone(&self.starts[index]);
        let cell = start.cell();
        if c.sequential {
            self.stats.wanted[cell.index()] += 1;
        }
        self.stats.started[cell.index()] += 1;
        let hp_cap_pct = if cell.room == EncounterRoom::Monster && !c.sequential {
            c.hall_hp_pct
        } else {
            0
        };
        Some(GymTicket {
            start,
            index,
            seed,
            variant,
            hp_cap_pct,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RunConfig, RunEnd, RunEnvs, RunOptions, SeedSource};

    fn record(act: u8, act_id: &str, encounter: &str, floor: usize) -> String {
        format!(
            r#"{{"act":{act},"act_id":"{act_id}","encounter":"{encounter}","floor":{floor},"hp":41,"max_hp":77,"gold":123,"deck":[["STRIKE_NECROBINDER",0],["STRIKE_NECROBINDER",1],["DEFEND_NECROBINDER",0],["DEFEND_NECROBINDER",0],["BODYGUARD",1],["UNLEASH",0]],"relics":["BOUND_PHYLACTERY"],"potion_slots":2,"potions":["BLOCK_POTION"]}}"#
        )
    }

    #[test]
    fn builds_the_fight_of_each_act_and_room() {
        for (act, id, enc, floor, room) in [
            (0, "OVERGROWTH", "BYRDONIS_ELITE", 9, RoomType::Elite),
            (0, "UNDERDOCKS", "SOUL_FYSH_BOSS", 17, RoomType::Boss),
            (1, "HIVE", "KAISER_CRAB_BOSS", 34, RoomType::Boss),
            (1, "HIVE", "ENTOMANCER_ELITE", 25, RoomType::Elite),
            (2, "GLORY", "QUEEN_BOSS", 51, RoomType::Boss),
            (2, "GLORY", "KNIGHTS_ELITE", 40, RoomType::Elite),
        ] {
            let start = HumanStart::from_json(&record(act, id, enc, floor)).expect("parses");
            for seed in [1, 2, 3] {
                let run = start.build(seed, Variant::default(), 0).expect("builds");
                let combat = run.combat().expect("in the fight");
                assert_eq!(combat.state().encounter.entry(), enc);
                let s = run.state();
                assert_eq!(s.act_index, usize::from(act));
                assert_eq!(s.total_floor(), floor);
                assert_eq!(s.max_hp, 77);
                let cs = combat.state();
                assert_eq!(cs.creature(cs.player.creature).hp, 41, "{enc}");
                assert_eq!(s.deck.len(), 6);
                assert_eq!(s.potions, vec![Some("BLOCK_POTION"), None]);
                assert!(
                    matches!(s.room, sts2_core::run::Room::Combat { room_type, .. } if room_type == room)
                );
            }
        }
    }

    #[test]
    fn rejects_unported_content() {
        let bad = record(1, "HIVE", "KAISER_CRAB_BOSS", 34).replace("UNLEASH", "NOT_A_CARD");
        assert!(HumanStart::from_json(&bad).unwrap_err().starts_with("card"));
        let bad = record(1, "HIVE", "QUEEN_BOSS", 34);
        assert!(
            HumanStart::from_json(&bad)
                .unwrap_err()
                .starts_with("encounter")
        );
    }

    #[test]
    fn start_record_round_trips() {
        let line = record(1, "HIVE", "KAISER_CRAB_BOSS", 34);
        let start = HumanStart::from_json(&line).unwrap();
        let run = start.build(9, Variant::default(), 0).unwrap();
        let out = start_record(&run).expect("in a fight");
        let back = HumanStart::from_json(&out).expect("parses");
        assert_eq!(back.encounter.id, "KAISER_CRAB_BOSS");
        assert_eq!(
            (back.act, back.floor, back.hp, back.max_hp),
            (1, 34, 41, 77)
        );
        assert_eq!(back.deck.len(), 6);
        assert_eq!(back.deck[1].upgrade_level, 1);
        assert_eq!(back.relics, start.relics);
        assert_eq!(back.potions, vec!["BLOCK_POTION"]);
        assert_eq!(back.potion_slots, 2);
    }

    #[test]
    fn variants_and_hall_cap_set_hp() {
        let start = HumanStart::from_json(&record(1, "HIVE", "KAISER_CRAB_BOSS", 34)).unwrap();
        let v = Variant {
            hp_permille: 1000,
            card: Some((0, true)),
        };
        let run = start.build(5, v, 0).unwrap();
        assert_eq!(run.state().hp, 77);
        assert_eq!(run.state().deck[0].upgrade_level, 1);
        let run = start.build(5, Variant::default(), 40).unwrap();
        assert_eq!(run.state().hp, 30);
    }

    fn envs(n: usize) -> RunEnvs {
        let config = RunConfig {
            ascension: 10,
            options: RunOptions::E2E,
            ..RunConfig::default()
        };
        RunEnvs::new(
            n,
            SeedSource::Range {
                start: 100,
                end: 1000,
                cycle: true,
            },
            config,
        )
    }

    #[test]
    fn gym_slots_play_terminal_fights_and_off_is_unchanged() {
        let text = [
            record(1, "HIVE", "KAISER_CRAB_BOSS", 34),
            record(0, "OVERGROWTH", "BYRDONIS_ELITE", 9),
        ]
        .join("\n");
        // Loaded with no slots: the same as never loaded.
        let mut a = envs(8);
        let mut b = envs(8);
        b.load_gym(Gym::parse(&text).unwrap());
        b.set_gym(None);
        for _ in 0..50 {
            let acts: Vec<u16> = (0..8).map(|i| first_legal(&a, i)).collect();
            a.step(&acts).unwrap();
            b.step(&acts).unwrap();
            assert_eq!(a.obs(), b.obs());
        }
        // Sequential: env 2 and 3 take the two records.
        let mut g = envs(4);
        g.load_gym(Gym::parse(&text).unwrap());
        g.set_gym(Some(GymConfig {
            first: 2,
            slots: 2,
            sequential: true,
            seed: 7,
            ..GymConfig::default()
        }));
        g.reset_all();
        assert_eq!(&g.cells()[..4], &[0, 0, 7 + 4, 7]);
        let mut ended = [false; 4];
        for _ in 0..3000 {
            let acts: Vec<u16> = (0..4).map(|i| first_legal(&g, i)).collect();
            let mut ends = Vec::new();
            g.step_with(&acts, |e| ends.push((e.env, e.end))).unwrap();
            for (i, end) in ends {
                if i >= 2 && !ended[i] {
                    ended[i] = true;
                    assert!(
                        matches!(
                            end,
                            RunEnd::GymEnd | RunEnd::Died | RunEnd::Victory | RunEnd::FightCapped
                        ),
                        "{end:?}"
                    );
                }
            }
            if ended[2] && ended[3] {
                break;
            }
        }
        assert!(ended[2] && ended[3]);
        assert_eq!(g.gym().unwrap().stats().started[4], 1);
    }

    fn first_legal(e: &RunEnvs, i: usize) -> u16 {
        let m = e.mask(i);
        (0..m.len() * 64)
            .find(|&k| m[k / 64] >> (k % 64) & 1 == 1)
            .map_or(crate::HOLD, |k| u16::try_from(k).unwrap())
    }
}
