//! A stronger policy used to look for one reachable victory (RLB3).
//!
//! Fights are played by a beam search over play sequences, each candidate scored at the start of
//! the next player turn after running the enemies' turn on a copy. The search keeps the best
//! plan for the turn, so a run is deterministic. Out of combat the policy prefers events, shops
//! and rest sites, picks a small number of good cards, spends gold, and judges events and
//! ancients by the state they leave behind.

use std::collections::VecDeque;
use std::fmt::Write as _;

use sts2_core::Character;
use sts2_core::Combat;
use sts2_core::combat::{
    Action, CardId, CombatResult, Decision, DecisionRequest, PromptText, Side,
};
use sts2_core::content::cards::{CardRarity, CardType};
use sts2_core::content::powers::PowerType;
use sts2_core::run::{
    Act1, CardSelect, MapCoord, MapPointType, RestOptionKind, Reward, Room, RoomType, Run,
    RunAction, RunCard, RunDecision, SelectPurpose, UnlockState, shop,
};

/// The number of partial plans kept at each depth.
const BEAM: usize = 8;
/// The most plays one turn's plan holds.
const MAX_PLAYS: usize = 12;
/// The player's HP is worth this many points in a fight.
const HP_WEIGHT: i64 = 100;
/// An enemy's HP is worth this many points (it is a cost, not a gain).
const ENEMY_HP_WEIGHT: i64 = 30;
/// Killing an enemy is worth this many points.
const KILL_WEIGHT: i64 = 600;
/// A point of enemy debuff is worth this many points.
const DEBUFF_WEIGHT: i64 = 5;
/// A pet's HP plus block is worth this many points.
const PET_WEIGHT: i64 = 4;
/// A point of a player buff is worth this many points.
const BUFF_WEIGHT: i64 = 4;
/// One potion in the plan costs this much: a resource, not free.
const POTION_COST: i64 = 15;
/// Playing a power is worth more than the turn shows: its effect runs for the rest of the fight.
const POWER_BONUS: i64 = 120;
/// The fewest HP (percent) the map policy wants before it takes an elite.
const ELITE_HP_PERCENT: i32 = 85;
/// Rest instead of upgrading below this share of max HP (in percent).
const REST_HP_PERCENT: i32 = 75;
/// Before the boss, rest below this share of max HP (in percent).
const BOSS_REST_HP_PERCENT: i32 = 90;

/// What a `reach` run ended as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Victory,
    Death,
    Stopped,
    Stuck,
    Cap,
}

impl End {
    /// The word `reach` prints.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Victory => "victory",
            Self::Death => "death",
            Self::Stopped => "stopped",
            Self::Stuck => "stuck",
            Self::Cap => "cap",
        }
    }
}

/// What one run did, with the actions it took.
#[derive(Clone, Debug)]
pub struct Played {
    pub outcome: Outcome,
    pub actions: Vec<RunAction>,
}

/// What one run did.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub end: End,
    pub act: usize,
    pub floor: usize,
    pub hp: i32,
    pub max_hp: i32,
    pub gold: i32,
    pub deck: usize,
    pub relics: usize,
    pub actions: usize,
    /// The reason a run stopped or got stuck.
    pub reason: String,
}

/// The combat policy: a beam search over play sequences, planning one turn at a time.
#[derive(Clone, Debug, Default)]
pub struct Search {
    plan: VecDeque<Action>,
}

impl Search {
    /// The next action in `run`'s fight from `modeled` (its modeled actions).
    pub fn choose(&mut self, run: &Run, combat: &Combat, modeled: &[RunAction]) -> RunAction {
        if let Some(next) = self.plan.pop_front() {
            let action = RunAction::Combat(next);
            if modeled.contains(&action) {
                return action;
            }
        }
        let big = matches!(
            run.state().room,
            Room::Combat {
                room_type: RoomType::Elite | RoomType::Boss,
                ..
            }
        );
        let first: Vec<Action> = modeled
            .iter()
            .filter_map(|a| match a {
                RunAction::Combat(a) => Some(a.clone()),
                _ => None,
            })
            .collect();
        self.plan = plan_turn(combat, big, &first).into();
        let end_turn = RunAction::Combat(Action::EndTurn);
        match self.plan.pop_front() {
            Some(a) => RunAction::Combat(a),
            None if modeled.contains(&end_turn) => end_turn,
            None => modeled[0].clone(),
        }
    }

    /// Forgets a planned turn.
    pub fn clear(&mut self) {
        self.plan.clear();
    }

    /// The answer to a card choice inside combat.
    #[must_use]
    pub fn answer(combat: &Combat) -> Option<Action> {
        answer_choice(combat)
    }
}

/// Plays one whole run with the search policy.
///
/// # Errors
/// If the run cannot be created.
pub fn play(
    character: Character,
    seed: &str,
    ascension: u8,
    act1: Act1,
    max_actions: usize,
    mut log: impl FnMut(&Run),
) -> Result<Played, String> {
    let mut run = Run::new(character, seed, ascension, UnlockState::all(), act1)
        .map_err(|e| e.to_string())?
        .with_events();
    let mut search = Search::default();
    let mut actions = 0;
    let mut taken = Vec::new();
    let end = loop {
        log(&run);
        match run.pending_decision() {
            RunDecision::Victory => break End::Victory,
            RunDecision::GameOver => break End::Death,
            RunDecision::Stopped(_) | RunDecision::Unsupported(_) => break End::Stopped,
            _ => {}
        }
        if actions >= max_actions {
            break End::Cap;
        }
        let mut failed: Vec<RunAction> = Vec::new();
        let mut accepted = false;
        while !accepted {
            let legal: Vec<RunAction> = run
                .legal_actions()
                .into_iter()
                .filter(|a| !failed.contains(a))
                .collect();
            if legal.is_empty() {
                return Ok(Played {
                    outcome: outcome(&run, End::Stuck, actions, "no legal action".to_owned()),
                    actions: taken,
                });
            }
            let action = if let Some(combat) = run.combat() {
                combat.pending_decision().map_or_else(
                    || search.choose(&run, combat, &legal),
                    |_| Search::answer(combat).map_or_else(|| legal[0].clone(), RunAction::Combat),
                )
            } else {
                search.clear();
                choose_run_action(&run, &legal)
            };
            debug_combat(&run, &action);
            match run.apply(action.clone()) {
                Ok(()) => {
                    actions += 1;
                    accepted = true;
                    taken.push(action);
                }
                Err(e) => {
                    failed.push(action);
                    let _ = e;
                }
            }
        }
    };
    Ok(Played {
        outcome: outcome(&run, end, actions, String::new()),
        actions: taken,
    })
}

/// Plays a recorded action list on a fresh run.
///
/// # Errors
/// If the run cannot be created or an action no longer applies.
pub fn replay(
    character: Character,
    seed: &str,
    ascension: u8,
    act1: Act1,
    actions: &[RunAction],
) -> Result<Outcome, String> {
    let mut run = Run::new(character, seed, ascension, UnlockState::all(), act1)
        .map_err(|e| e.to_string())?
        .with_events();
    for (i, action) in actions.iter().enumerate() {
        match run.pending_decision() {
            RunDecision::Victory | RunDecision::GameOver => break,
            _ => {}
        }
        run.apply(action.clone())
            .map_err(|e| format!("action {} ({action:?}): {e}", i + 1))?;
    }
    let end = match run.pending_decision() {
        RunDecision::Victory => End::Victory,
        RunDecision::GameOver => End::Death,
        RunDecision::Stopped(_) | RunDecision::Unsupported(_) => End::Stopped,
        _ => End::Stuck,
    };
    Ok(outcome(&run, end, actions.len(), String::new()))
}

/// The fixture text: a header and one action per line.
#[must_use]
pub fn fixture_text(
    character: Character,
    seed: &str,
    ascension: u8,
    act1: Act1,
    actions: &[RunAction],
) -> String {
    let mut out = format!(
        "# spirebot rlb3 victory replay\nseed {seed}\ncharacter {}\nascension {ascension}\nact1 {}\nactions {}\n",
        character.name(),
        act1.key(),
        actions.len()
    );
    for action in actions {
        let _ = writeln!(out, "{}", action_line(action));
    }
    out
}

/// A recorded fixture.
#[derive(Clone, Debug)]
pub struct Fixture {
    pub character: Character,
    pub seed: String,
    pub ascension: u8,
    pub act1: Act1,
    pub actions: Vec<RunAction>,
}

impl Fixture {
    /// Reads [`fixture_text`]'s format.
    ///
    /// # Errors
    /// If a header line is missing or an action line is malformed.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut character = None;
        let mut seed = None;
        let mut ascension = None;
        let mut act1 = None;
        let mut count = None;
        let mut actions = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once(' ') else {
                actions.push(parse_action(line)?);
                continue;
            };
            match key {
                "seed" => seed = Some(value.to_owned()),
                "character" => {
                    character = Some(
                        Character::from_name(value)
                            .ok_or_else(|| format!("unknown character {value}"))?,
                    );
                }
                "ascension" => {
                    ascension = Some(value.parse().map_err(|_| "bad ascension".to_owned())?);
                }
                "act1" => {
                    act1 = Some(Act1::from_key(value).ok_or_else(|| format!("bad act1 {value}"))?);
                }
                "actions" => {
                    count = Some(
                        value
                            .parse::<usize>()
                            .map_err(|_| "bad actions".to_owned())?,
                    );
                }
                _ => actions.push(parse_action(line)?),
            }
        }
        let actions_len = actions.len();
        if count.is_some_and(|n| n != actions_len) {
            return Err(format!(
                "the fixture says {count:?} actions but has {actions_len}"
            ));
        }
        Ok(Self {
            character: character.ok_or("no character line")?,
            seed: seed.ok_or("no seed line")?,
            ascension: ascension.ok_or("no ascension line")?,
            act1: act1.ok_or("no act1 line")?,
            actions,
        })
    }
}

/// One action as a line of a fixture.
#[must_use]
pub fn action_line(action: &RunAction) -> String {
    match action {
        RunAction::AncientOption(i) => format!("ancient {i}"),
        RunAction::EventOption(i) => format!("event {i}"),
        RunAction::ChooseNode(c) => format!("node {} {}", c.col, c.row),
        RunAction::Combat(a) => combat_line(a),
        RunAction::ClaimReward(i) => format!("claim {i}"),
        RunAction::PickCard { reward, card } => format!("pickcard {reward} {card}"),
        RunAction::SkipReward(i) => format!("skip {i}"),
        RunAction::CardAlternative {
            reward,
            alternative,
        } => format!("alternative {reward} {alternative}"),
        RunAction::RestOption(i) => format!("rest {i}"),
        RunAction::UsePotion(i) => format!("usepotion {i}"),
        RunAction::DiscardPotion(i) => format!("discardpotion {i}"),
        RunAction::SelectCard(i) => format!("select {i}"),
        RunAction::DeselectCard(i) => format!("deselect {i}"),
        RunAction::ConfirmSelect => "confirm".to_owned(),
        RunAction::CancelSelect => "cancel".to_owned(),
        RunAction::TakeRelic => "take".to_owned(),
        RunAction::BuyCard(i) => format!("buycard {i}"),
        RunAction::BuyRelic(i) => format!("buyrelic {i}"),
        RunAction::BuyPotion(i) => format!("buypotion {i}"),
        RunAction::RemoveCard => "remove".to_owned(),
        RunAction::SelectBundle(i) => format!("bundle {i}"),
        RunAction::Proceed => "proceed".to_owned(),
    }
}

/// The line of a combat action.
fn combat_line(action: &Action) -> String {
    match action {
        Action::PlayCard { card, target } => {
            format!(
                "play {} {}",
                card.0,
                target.map_or("-".to_owned(), |t| t.0.to_string())
            )
        }
        Action::UsePotion { slot, target } => format!(
            "potion {slot} {}",
            target.map_or("-".to_owned(), |t| t.0.to_string())
        ),
        Action::EndTurn => "end".to_owned(),
        Action::Resolve(Decision::Cards(cards)) => {
            let ids: Vec<String> = cards.iter().map(|c| c.0.to_string()).collect();
            format!(
                "cards {}",
                if ids.is_empty() {
                    "-".to_owned()
                } else {
                    ids.join(",")
                }
            )
        }
        Action::Resolve(Decision::Pick(card)) => format!("resolvepick {}", card.0),
        Action::Resolve(Decision::Done) => "resolvedone".to_owned(),
        Action::Resolve(Decision::Unpick(card)) => format!("resolveunpick {}", card.0),
    }
}

/// Parses one line of a fixture.
///
/// # Errors
/// If the line is not a known action.
pub fn parse_action(line: &str) -> Result<RunAction, String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let int = |i: usize| -> Result<usize, String> {
        parts
            .get(i)
            .ok_or_else(|| format!("missing number in {line:?}"))?
            .parse()
            .map_err(|_| format!("bad number in {line:?}"))
    };
    let uint = |i: usize| -> Result<u32, String> {
        parts
            .get(i)
            .ok_or_else(|| format!("missing number in {line:?}"))?
            .parse()
            .map_err(|_| format!("bad number in {line:?}"))
    };
    let target = |i: usize| -> Result<Option<sts2_core::combat::CreatureId>, String> {
        match parts.get(i).copied() {
            Some("-") | None => Ok(None),
            Some(n) => Ok(Some(sts2_core::combat::CreatureId(
                n.parse().map_err(|_| format!("bad target in {line:?}"))?,
            ))),
        }
    };
    Ok(match parts.first().copied() {
        Some("ancient") => RunAction::AncientOption(int(1)?),
        Some("event") => RunAction::EventOption(int(1)?),
        Some("node") => RunAction::ChooseNode(MapCoord {
            col: int(1)?,
            row: int(2)?,
        }),
        Some("play") => RunAction::Combat(Action::PlayCard {
            card: CardId(
                parts
                    .get(1)
                    .ok_or("missing card")?
                    .parse()
                    .map_err(|_| "bad card")?,
            ),
            target: target(2)?,
        }),
        Some("potion") => RunAction::Combat(Action::UsePotion {
            slot: int(1)?,
            target: target(2)?,
        }),
        Some("end") => RunAction::Combat(Action::EndTurn),
        Some("cards") => {
            let ids = parts.get(1).copied().unwrap_or("-");
            let cards: Result<Vec<CardId>, String> = if ids == "-" {
                Ok(Vec::new())
            } else {
                ids.split(',')
                    .map(|id| id.parse().map(CardId).map_err(|_| "bad card id".to_owned()))
                    .collect()
            };
            RunAction::Combat(Action::Resolve(Decision::Cards(cards?)))
        }
        Some("resolvepick") => RunAction::Combat(Action::Resolve(Decision::Pick(CardId(uint(1)?)))),
        Some("resolveunpick") => {
            RunAction::Combat(Action::Resolve(Decision::Unpick(CardId(uint(1)?))))
        }
        Some("resolvedone") => RunAction::Combat(Action::Resolve(Decision::Done)),
        Some("claim") => RunAction::ClaimReward(int(1)?),
        Some("pickcard") => RunAction::PickCard {
            reward: int(1)?,
            card: int(2)?,
        },
        Some("skip") => RunAction::SkipReward(int(1)?),
        Some("alternative") => RunAction::CardAlternative {
            reward: int(1)?,
            alternative: int(2)?,
        },
        Some("rest") => RunAction::RestOption(int(1)?),
        Some("usepotion") => RunAction::UsePotion(int(1)?),
        Some("discardpotion") => RunAction::DiscardPotion(int(1)?),
        Some("select") => RunAction::SelectCard(int(1)?),
        Some("deselect") => RunAction::DeselectCard(int(1)?),
        Some("confirm") => RunAction::ConfirmSelect,
        Some("cancel") => RunAction::CancelSelect,
        Some("take") => RunAction::TakeRelic,
        Some("buycard") => RunAction::BuyCard(int(1)?),
        Some("buyrelic") => RunAction::BuyRelic(int(1)?),
        Some("buypotion") => RunAction::BuyPotion(int(1)?),
        Some("remove") => RunAction::RemoveCard,
        Some("bundle") => RunAction::SelectBundle(int(1)?),
        Some("proceed") => RunAction::Proceed,
        _ => return Err(format!("unknown action line {line:?}")),
    })
}

const fn outcome(run: &Run, end: End, actions: usize, reason: String) -> Outcome {
    let s = run.state();
    Outcome {
        end,
        act: s.act_index + 1,
        floor: s.total_floor(),
        hp: s.hp,
        max_hp: s.max_hp,
        gold: s.gold,
        deck: s.deck.len(),
        relics: s.relics.len(),
        actions,
        reason,
    }
}

/// Prints one combat step when `STS2_REACH_DEBUG` is set (a development aid, not an interface).
#[allow(clippy::print_stderr, reason = "the debug aid writes to stderr")]
fn debug_combat(run: &Run, action: &RunAction) {
    if std::env::var_os("STS2_REACH_DEBUG").is_none() {
        return;
    }
    let Some(combat) = run.combat() else {
        return;
    };
    let s = combat.state();
    let p = s.creature(s.player.creature);
    let hand: Vec<String> = s
        .player
        .hand
        .iter()
        .map(|&c| format!("{:?}", s.card(c).kind))
        .collect();
    let enemies: Vec<String> = s
        .enemies
        .iter()
        .map(|&c| {
            let e = s.creature(c);
            format!(
                "{:?} {}/{}",
                e.monster.as_ref().map(|m| m.kind),
                e.hp,
                e.max_hp
            )
        })
        .collect();
    eprintln!(
        "r{} hp {}/{} e{} hand {:?} enemies {:?} -> {action:?}",
        s.player.turn_number, p.hp, p.max_hp, s.player.energy, hand, enemies
    );
}

// ---------------------------------------------------------------------------------------------
// Out of combat.

/// The best run-level choice among `modeled`.
fn choose_run_action(run: &Run, modeled: &[RunAction]) -> RunAction {
    let pick = match run.pending_decision() {
        RunDecision::Ancient(room) => {
            // A locked option is never legal; an unsupported one is counted out by the value.
            best_option(run, modeled, |a| matches!(a, RunAction::AncientOption(_)))
                .or_else(|| Some(RunAction::AncientOption(room.options.len() - 1)))
        }
        RunDecision::Event(_) => {
            best_option(run, modeled, |a| matches!(a, RunAction::EventOption(_)))
        }
        RunDecision::Map(_) => map_choice(run, modeled),
        RunDecision::Rewards(rewards) => reward_choice(run, rewards, modeled),
        RunDecision::CardSelect(select) => card_select_choice(run, select, modeled),
        RunDecision::Bundles(bundles) => bundles
            .iter()
            .enumerate()
            .filter(|(i, _)| modeled.contains(&RunAction::SelectBundle(*i)))
            .max_by_key(|(_, cards)| cards.iter().map(|c| card_quality(*c)).sum::<i32>())
            .map(|(i, _)| RunAction::SelectBundle(i)),
        RunDecision::RestSite(options) => rest_choice(run, options, modeled),
        RunDecision::Treasure(Some(_)) => Some(RunAction::TakeRelic)
            .filter(|a| modeled.contains(a))
            .or(Some(RunAction::Proceed)),
        RunDecision::Treasure(None) => Some(RunAction::Proceed),
        RunDecision::Shop(shop) => Some(shop_choice(run, shop, modeled)),
        _ => None,
    };
    pick.unwrap_or_else(|| {
        // Never drink a potion out of combat by accident.
        modeled
            .iter()
            .find(|a| !matches!(a, RunAction::UsePotion(_) | RunAction::DiscardPotion(_)))
            .unwrap_or(&modeled[0])
            .clone()
    })
}

/// The option whose immediate outcome on a copy of the run is best.
fn best_option(
    run: &Run,
    modeled: &[RunAction],
    want: impl Fn(&RunAction) -> bool,
) -> Option<RunAction> {
    let now = run_value(run);
    modeled
        .iter()
        .filter(|a| want(a) && !matches!(a, RunAction::UsePotion(_)))
        .filter_map(|a| {
            let mut copy = run.clone();
            copy.apply(a.clone()).ok()?;
            Some((run_value(&copy) - now, a))
        })
        .rev()
        .max_by_key(|(score, _)| *score)
        .map(|(_, a)| a.clone())
}

/// What a state is worth outside a fight: HP and max HP first, then relics, deck quality and
/// gold. A run that ended is worth far less.
fn run_value(run: &Run) -> i64 {
    let s = run.state();
    if run.pending_decision() == RunDecision::GameOver {
        return -1_000_000;
    }
    if matches!(
        run.pending_decision(),
        RunDecision::Stopped(_) | RunDecision::Unsupported(_)
    ) {
        return -500_000;
    }
    let deck: i64 = s.deck.iter().map(|c| i64::from(card_quality(*c)) - 2).sum();
    let relics: i64 = s.relics.iter().map(|r| 30 + relic_bonus(r.kind.id())).sum();
    let potions =
        i64::try_from(s.potions.iter().filter(|p| p.is_some()).count()).unwrap_or(i64::MAX);
    3 * i64::from(s.max_hp)
        + i64::from(s.hp)
        + i64::from(s.gold) / 8
        + relics
        + deck
        + 8 * potions
        + if matches!(run.pending_decision(), RunDecision::Victory) {
            10_000
        } else {
            0
        }
}

/// A crude quality of a run card: rarity, type and cost, with curses strongly negative.
fn card_quality(card: RunCard) -> i32 {
    let Some(kind) = card.kind() else {
        return 0;
    };
    if kind.card_type() == CardType::Curse {
        return -20;
    }
    if matches!(kind.card_type(), CardType::Status | CardType::Quest) {
        return -5;
    }
    let rarity = match kind.rarity() {
        CardRarity::Rare | CardRarity::Ancient => 9,
        CardRarity::Uncommon => 4,
        CardRarity::Event => 3,
        _ => 0,
    };
    let kind_bonus = match kind.card_type() {
        CardType::Power => 4,
        CardType::Attack => 3,
        CardType::Skill => 2,
        _ => 0,
    };
    let cost = kind.cost();
    let cost_bonus = if cost <= 0 {
        1
    } else if cost >= 3 {
        1 - cost
    } else {
        0
    };
    rarity + kind_bonus + cost_bonus + i32::from(card.upgrade_level)
}

/// A value beyond the generic relic count for relics whose payoff is not in the immediate state.
fn relic_bonus(id: &str) -> i64 {
    match id {
        "TOY_BOX" => 140,
        "GOLDEN_COMPASS" => 70,
        "VAJRA" | "VAMBRACE" | "PLANISPHERE" => 35,
        "STRAWBERRY" | "LASTING_CANDY" | "MEAL_TICKET" | "LEES_WAFFLE" => 25,
        "REPTILE_TRINKET" | "PANTOGRAPH" | "POTION_BELT" | "BOOK_OF_FIVE_RINGS" => 20,
        _ => 0,
    }
}

/// The map: the child on the best path to the boss. Events, shops and rest sites count for a
/// path, elites against it when hurt.
fn map_choice(run: &Run, modeled: &[RunAction]) -> Option<RunAction> {
    let s = run.state();
    let hurt = s.hp * 100 < s.max_hp * ELITE_HP_PERCENT;
    let worth = |kind: MapPointType| -> i32 {
        match kind {
            MapPointType::Unknown => 5,
            MapPointType::RestSite | MapPointType::Shop => 4,
            MapPointType::Treasure => 3,
            MapPointType::Elite if hurt => -8,
            MapPointType::Monster | MapPointType::Elite => 1,
            _ => 0,
        }
    };
    let mut best: std::collections::HashMap<MapCoord, i32> = std::collections::HashMap::new();
    for p in s.map.points().iter().rev() {
        let rest = p
            .children
            .iter()
            .filter_map(|c| best.get(&c))
            .max()
            .copied()
            .unwrap_or(0);
        best.insert(p.coord, worth(p.kind) + rest);
    }
    modeled
        .iter()
        .filter_map(|a| match a {
            RunAction::ChooseNode(c) => s.map.point(*c).map(|p| {
                let now = match p.kind {
                    MapPointType::RestSite if hurt => 100,
                    MapPointType::Elite if hurt => -100,
                    _ => 0,
                };
                (now + best.get(c).copied().unwrap_or(0), a)
            }),
            _ => None,
        })
        .rev()
        .max_by_key(|(worth, _)| *worth)
        .map(|(_, a)| a.clone())
}

/// A rewards screen: claim everything valuable, pick a good card or skip it, then leave.
fn reward_choice(run: &Run, rewards: &[Reward], modeled: &[RunAction]) -> Option<RunAction> {
    let s = run.state();
    let has_slot = s.potions.contains(&None);
    for (i, reward) in rewards.iter().enumerate() {
        match reward {
            Reward::Card(cards) => {
                let threshold = if s.deck.len() < 14 { 3 } else { 5 };
                let best = cards
                    .cards
                    .iter()
                    .enumerate()
                    .filter(|(card, _)| {
                        modeled.contains(&RunAction::PickCard {
                            reward: i,
                            card: *card,
                        })
                    })
                    .max_by_key(|(_, c)| card_quality(**c));
                if let Some((card, c)) = best
                    && card_quality(*c) >= threshold
                    && s.deck.len() < 30
                {
                    return Some(RunAction::PickCard { reward: i, card });
                }
                return Some(RunAction::SkipReward(i))
                    .filter(|a| modeled.contains(a))
                    .or(Some(RunAction::Proceed));
            }
            Reward::Potion(_) if !has_slot => {
                return Some(RunAction::SkipReward(i))
                    .filter(|a| modeled.contains(a))
                    .or(Some(RunAction::Proceed));
            }
            _ => {
                let claim = RunAction::ClaimReward(i);
                if modeled.contains(&claim) {
                    return Some(claim);
                }
            }
        }
    }
    Some(RunAction::Proceed)
}

/// A deck card selection: upgrade the best card, remove the worst, otherwise take the best.
fn card_select_choice(run: &Run, select: &CardSelect, modeled: &[RunAction]) -> Option<RunAction> {
    let remove = matches!(
        select.purpose,
        SelectPurpose::Remove { .. } | SelectPurpose::Cook | SelectPurpose::ShopRemoval { .. }
    );
    let upgrade = matches!(
        select.purpose,
        SelectPurpose::Upgrade | SelectPurpose::Smith | SelectPurpose::Enchant { .. }
    );
    // One card at a time; the caller comes back until the selection is complete.
    let chosen = select
        .options
        .iter()
        .enumerate()
        .filter(|(i, _)| !select.picked.contains(i))
        .filter(|(i, _)| modeled.contains(&RunAction::SelectCard(*i)))
        .filter_map(|(i, _)| {
            let card = selected_card(run, select, i)?;
            let mut quality = card_quality(card);
            if upgrade && card.upgrade_level == 0 {
                quality += match card.kind()?.card_type() {
                    CardType::Attack | CardType::Power => 2,
                    _ => 0,
                };
            }
            Some((i, quality))
        })
        .max_by_key(|(_, quality)| if remove { -*quality } else { *quality });
    if let Some((i, _)) = chosen {
        return Some(RunAction::SelectCard(i));
    }
    Some(RunAction::ConfirmSelect)
        .filter(|a| modeled.contains(a))
        .or_else(|| Some(RunAction::CancelSelect).filter(|a| modeled.contains(a)))
        .or_else(|| Some(RunAction::Proceed).filter(|a| modeled.contains(a)))
}

/// The card a selection option stands for: a deck index or an index into `offered`.
fn selected_card(run: &Run, select: &CardSelect, option: usize) -> Option<RunCard> {
    let index = *select.options.get(option)?;
    if select.offered.is_empty() {
        run.state().deck.get(index).copied()
    } else {
        select.offered.get(index).copied()
    }
}

/// The rest site: heal when hurt, hatch an egg when there is one, otherwise smith.
fn rest_choice(
    run: &Run,
    options: &[sts2_core::run::RestOption],
    modeled: &[RunAction],
) -> Option<RunAction> {
    let s = run.state();
    let before_boss = s
        .coord()
        .is_some_and(|c| c.row + 1 == s.map.boss().coord.row);
    let threshold = if before_boss {
        BOSS_REST_HP_PERCENT
    } else {
        REST_HP_PERCENT
    };
    let hurt = s.hp * 100 < s.max_hp * threshold;
    let choose = |kind: RestOptionKind| {
        options
            .iter()
            .position(|o| o.kind == kind && o.enabled)
            .map(RunAction::RestOption)
            .filter(|a| modeled.contains(a))
    };
    if hurt && let Some(a) = choose(RestOptionKind::Heal) {
        return Some(a);
    }
    if let Some(a) = choose(RestOptionKind::Hatch) {
        return Some(a);
    }
    if let Some(a) = choose(RestOptionKind::Smith) {
        return Some(a);
    }
    if let Some(a) = choose(RestOptionKind::Dig).or_else(|| choose(RestOptionKind::Lift)) {
        return Some(a);
    }
    if let Some(a) = choose(RestOptionKind::Heal) {
        return Some(a);
    }
    Some(RunAction::Proceed)
        .filter(|a| modeled.contains(a))
        .or_else(|| {
            options
                .iter()
                .position(|o| o.enabled)
                .map(RunAction::RestOption)
        })
}

/// A shop: buy relics, then good cards, potions and a removal; otherwise leave.
fn shop_choice(run: &Run, shop: &shop::Shop, modeled: &[RunAction]) -> RunAction {
    if let Some(i) = shop
        .relics
        .iter()
        .enumerate()
        .filter(|(i, r)| r.id.is_some() && modeled.contains(&RunAction::BuyRelic(*i)))
        .max_by_key(|(_, r)| r.id.map_or(0, relic_bonus) + 10)
        .map(|(i, _)| i)
    {
        return RunAction::BuyRelic(i);
    }
    let s = run.state();
    if s.potions.contains(&None)
        && let Some(i) = shop
            .potions
            .iter()
            .enumerate()
            .filter(|(i, p)| p.id.is_some() && modeled.contains(&RunAction::BuyPotion(*i)))
            .max_by_key(|(_, p)| p.base_cost)
            .map(|(i, _)| i)
    {
        return RunAction::BuyPotion(i);
    }
    if let Some(i) = shop
        .cards
        .iter()
        .enumerate()
        .filter(|(i, c)| c.card.is_some() && modeled.contains(&RunAction::BuyCard(*i)))
        .filter(|(_, c)| c.card.is_some_and(|card| card_quality(card) >= 6))
        .max_by_key(|(_, c)| c.card.map_or(0, card_quality))
        .map(|(i, _)| i)
    {
        return RunAction::BuyCard(i);
    }
    if modeled.contains(&RunAction::RemoveCard)
        && s.deck.iter().any(|c| c.is_basic())
        && s.gold > shop::price(s, shop.removal_base_cost)
    {
        return RunAction::RemoveCard;
    }
    RunAction::Proceed
}

// ---------------------------------------------------------------------------------------------
// Combat search.

/// The plays of this turn, found by a beam search. Each candidate is scored after ending the
/// turn on a copy, at the start of the next player turn.
fn plan_turn(combat: &Combat, big_fight: bool, first: &[Action]) -> Vec<Action> {
    let before = Snapshot::of(combat.state());
    let potions_ok = big_fight || before.hp * 100 < before.max_hp * 60;
    let mut best = (eval(&end_turn_of(combat.clone())), Vec::new());
    let mut beam: Vec<(Combat, Vec<Action>)> = vec![(combat.clone(), Vec::new())];
    for depth in 0..MAX_PLAYS {
        let mut next: Vec<(i64, Combat, Vec<Action>)> = Vec::new();
        for (state, plan) in &beam {
            // The first play must be one the run models; later ones are checked when made.
            let legal = if depth == 0 {
                first.to_vec()
            } else {
                state.legal_actions()
            };
            for action in legal {
                let potion = matches!(action, Action::UsePotion { .. });
                if matches!(action, Action::EndTurn | Action::Resolve(_)) || (potion && !potions_ok)
                {
                    continue;
                }
                if potion && plan.iter().any(|a| matches!(a, Action::UsePotion { .. })) {
                    continue;
                }
                let power = matches!(&action, Action::PlayCard { card, .. }
                    if state.state().card(*card).kind.card_type() == CardType::Power);
                let mut copy = state.clone();
                if copy.apply(action.clone()).is_err() {
                    continue;
                }
                let copy = resolve_choices(copy);
                let mut plan = plan.clone();
                plan.push(action);
                let s = copy.state();
                if s.result == Some(CombatResult::Won) && player_alive(s) {
                    return plan;
                }
                if s.result == Some(CombatResult::Lost) || !player_alive(s) {
                    continue;
                }
                let mut score = eval(&end_turn_of(copy.clone()));
                if power {
                    score += POWER_BONUS;
                }
                if potion {
                    score -= POTION_COST;
                }
                next.push((score, copy, plan));
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort_by_key(|(score, ..)| std::cmp::Reverse(*score));
        next.truncate(BEAM);
        if next[0].0 > best.0 {
            best = (next[0].0, next[0].2.clone());
        }
        beam = next
            .into_iter()
            .map(|(_, state, plan)| (state, plan))
            .collect();
    }
    best.1
}

fn player_alive(s: &sts2_core::combat::CombatState) -> bool {
    s.creature(s.player.creature).hp > 0
}

/// The answers to a pending card choice, best cards first.
fn answer_choice(combat: &Combat) -> Option<Action> {
    let request = combat.pending_decision()?;
    let DecisionRequest::ChooseCards {
        options,
        min,
        max,
        prompt,
        ..
    } = request;
    let mut ranked: Vec<(i64, usize)> = options
        .iter()
        .enumerate()
        .map(|(i, &card)| (card_value(combat, card), i))
        .collect();
    let low_first = matches!(
        prompt.text,
        PromptText::Discard | PromptText::Exhaust | PromptText::Remove | PromptText::Transform
    );
    ranked.sort_by_key(|(v, ..)| if low_first { *v } else { -*v });
    let take = if low_first { *min } else { *max };
    let cards: Vec<_> = ranked
        .iter()
        .take(take.clamp(*min, *max))
        .map(|&(_, i)| options[i])
        .collect();
    Some(Action::Resolve(Decision::Cards(cards)))
}

/// A crude value of a card in hand for a choice that asks for one.
fn card_value(combat: &Combat, card: CardId) -> i64 {
    let c = combat.state().card(card);
    if c.kind.card_type() == CardType::Curse {
        return -100;
    }
    if matches!(c.kind.card_type(), CardType::Status | CardType::Quest) {
        return -50;
    }
    let rarity = match c.kind.rarity() {
        CardRarity::Rare | CardRarity::Ancient => 6,
        CardRarity::Uncommon => 3,
        _ => 1,
    };
    let kind = match c.kind.card_type() {
        CardType::Power => 6,
        CardType::Attack => 4,
        CardType::Skill => 3,
        _ => 2,
    };
    i64::from(rarity * 4 + kind - c.kind.cost())
}

/// Resolves any pending card choices on `combat`, choosing with [`answer_choice`].
fn resolve_choices(mut combat: Combat) -> Combat {
    while combat.pending_decision().is_some() {
        let Some(answer) = answer_choice(&combat) else {
            break;
        };
        if combat.apply(answer).is_err() {
            break;
        }
    }
    combat
}

/// Finishes any pending card choice, then ends the turn, on `combat`.
fn end_turn_of(combat: Combat) -> Combat {
    let mut combat = resolve_choices(combat);
    if combat.state().result.is_none() && combat.pending_decision().is_none() {
        let _ = combat.apply(Action::EndTurn);
    }
    combat
}

/// What the search compares at the start of a player turn.
struct Snapshot {
    over: bool,
    won: bool,
    hp: i32,
    max_hp: i32,
    enemy_hp: i32,
    enemies: i32,
    enemy_debuffs: i32,
    pets: i32,
    buffs: i32,
}

impl Snapshot {
    fn of(s: &sts2_core::combat::CombatState) -> Self {
        let player = s.creature(s.player.creature);
        let enemies: Vec<_> = s
            .enemies
            .iter()
            .map(|&id| s.creature(id))
            .filter(|c| c.is_alive())
            .collect();
        let buffs = player
            .powers
            .iter()
            .filter(|p| p.kind.power_type() == PowerType::Buff)
            .map(|p| p.amount.abs())
            .sum();
        Self {
            over: s.result.is_some(),
            won: s.result == Some(CombatResult::Won),
            hp: player.hp,
            max_hp: player.max_hp,
            enemy_hp: enemies.iter().map(|c| c.hp + c.block).sum(),
            enemies: i32::try_from(enemies.len()).unwrap_or(i32::MAX),
            enemy_debuffs: enemies
                .iter()
                .flat_map(|c| &c.powers)
                .filter(|p| p.kind.power_type() == PowerType::Debuff)
                .map(|p| p.amount.abs())
                .sum(),
            pets: s
                .allies
                .iter()
                .map(|&id| s.creature(id))
                .filter(|c| c.side == Side::Player && c.pet_owner.is_some() && c.is_alive())
                .map(|c| c.hp + c.block)
                .sum(),
            buffs,
        }
    }
}

/// The absolute value of a state at the start of a player turn.
fn eval(combat: &Combat) -> i64 {
    let s = Snapshot::of(combat.state());
    if s.over {
        return if s.won { i64::MAX / 2 } else { i64::MIN / 2 };
    }
    HP_WEIGHT * i64::from(s.hp)
        - ENEMY_HP_WEIGHT * i64::from(s.enemy_hp)
        - KILL_WEIGHT * i64::from(s.enemies)
        + DEBUFF_WEIGHT * i64::from(s.enemy_debuffs)
        + PET_WEIGHT * i64::from(s.pets)
        + BUFF_WEIGHT * i64::from(s.buffs)
}
