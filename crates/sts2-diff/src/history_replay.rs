//! `history-predict`'s floor-by-floor replay of a recorded run's Act 1 (`tools/replay_history.py`).
//!
//! A `.run` file has no combat log, so fights are never played. What it does pin down is every
//! offer the run saw: Neow's options, each fight's card, potion and relic rewards and gold, each
//! chest's relic and gold, each shop's stock, and which event each `?` became. Those come from
//! streams that combat does not touch (the player's `rewards` and `shops`, the run's
//! `treasure_room_relics`, the relic bags, the event list), so the engine's generators are called
//! on a [`RunState`] that is kept in step with the history: before each floor it gets the
//! recorded gold, HP, deck, belt and relics, and after it the bags lose every relic the floor
//! offered.
//!
//! Some consumers of the `rewards` stream are not modeled here: events (their own rewards, card
//! offers, relics), rest-site heals with a reward hook, and Neow options that fail to apply. After
//! one the chain is out of sync; the next fight or shop is matched by searching how many draws
//! the gap took (`RESYNC_DRAWS`), and checks made after such a resync are flagged `resynced`.

use std::collections::HashMap;

use serde_json::{Value, json};
use sts2_core::Character;
use sts2_core::content::relics::RelicKind;
use sts2_core::decimal::Decimal;
use sts2_core::rng::PlayerRngType;
use sts2_core::run::odds::{CardRarityOddsType, GoldProportion};
use sts2_core::run::rewards::{self, CardCreation, PendingReward, Reward};
use sts2_core::run::shop::Shop;
use sts2_core::run::{
    Act1, MapPointHistoryEntry, MapPointType, RoomType, Run, RunAction, RunCard, RunDecision,
    RunRelic, RunState, UnlockState, events, hooks,
};

use crate::Result;

/// The most `rewards` draws an unmodeled gap is searched for.
const RESYNC_DRAWS: usize = 400;

/// Interns ids from the history as the `&'static str` the engine's state holds.
#[derive(Debug, Default)]
struct Ids(HashMap<String, &'static str>);

impl Ids {
    fn get(&mut self, id: &str) -> &'static str {
        if let Some(&s) = self.0.get(id) {
            return s;
        }
        let s: &'static str = Box::leak(id.to_owned().into_boxed_str());
        self.0.insert(id.to_owned(), s);
        s
    }
}

/// `CARD.X` → `X`.
fn bare(id: &str) -> &str {
    id.split_once('.').map_or(id, |(_, rest)| rest)
}

/// A recorded card as `ID` or `ID+` (upgraded).
fn recorded_card(card: &Value) -> String {
    let id = bare(card["id"].as_str().unwrap_or(""));
    if card["current_upgrade_level"].as_u64().unwrap_or(0) > 0 {
        format!("{id}+")
    } else {
        id.to_owned()
    }
}

/// An engine card as `ID` or `ID+`.
fn engine_card(card: &RunCard) -> String {
    if card.upgrade_level > 0 {
        format!("{}+", card.id)
    } else {
        card.id.to_owned()
    }
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// The recorded ids of a `{choice, was_picked}` list (`relic_choices`, `potion_choices`).
fn choices(stats: &Value, key: &str) -> Vec<String> {
    stats[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| c["choice"].as_str())
                .map(|c| bare(c).to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The picked ids of a `{choice, was_picked}` list.
fn picked(stats: &Value, key: &str) -> Vec<String> {
    stats[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|c| c["was_picked"].as_bool() == Some(true))
                .filter_map(|c| c["choice"].as_str())
                .map(|c| bare(c).to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The ids of a plain id list (`bought_relics`, `potion_used`, ...).
fn ids(stats: &Value, key: &str) -> Vec<String> {
    stats[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|c| bare(c).to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The recorded cards offered on `card_choices`.
fn card_choices(stats: &Value) -> Vec<String> {
    stats["card_choices"]
        .as_array()
        .map(|a| a.iter().map(|c| recorded_card(&c["card"])).collect())
        .unwrap_or_default()
}

/// The cards of a list of card objects (`cards_gained`, `cards_removed`).
fn cards(stats: &Value, key: &str) -> Vec<String> {
    stats[key]
        .as_array()
        .map(|a| a.iter().map(recorded_card).collect())
        .unwrap_or_default()
}

/// What a set of populated rewards offers, by kind.
#[derive(Debug, Default, PartialEq, Eq)]
struct Offer {
    cards: Vec<String>,
    potions: Vec<String>,
    relics: Vec<String>,
    gold: i32,
}

impl Offer {
    fn of(rewards: &[Reward]) -> Self {
        let mut offer = Self::default();
        for reward in rewards {
            match reward {
                Reward::Gold(g) => offer.gold += g,
                Reward::Potion(p) => offer.potions.push((*p).to_owned()),
                Reward::Relic(r) | Reward::WaxRelic(r) => offer.relics.push((*r).to_owned()),
                Reward::Card(cards) => offer.cards.extend(cards.cards.iter().map(engine_card)),
                // A returned stolen card (`SpecialCardReward`) is not a card choice offered.
                Reward::SpecialCard(_) => {}
            }
        }
        offer
    }
}

/// The checks of one replay and where its `rewards` chain lost sync.
#[derive(Debug, Default)]
struct Report {
    checks: Vec<Value>,
    gaps: Vec<Value>,
}

impl Report {
    fn check(
        &mut self,
        floor: usize,
        field: &str,
        predicted: Value,
        recorded: Value,
        extra: &Value,
    ) -> bool {
        let ok = predicted == recorded;
        let mut c = json!({"floor": floor, "field": field,
            "status": if ok { "match" } else { "mismatch" }});
        if !ok {
            c["predicted"] = predicted;
            c["recorded"] = recorded;
        }
        if let (Some(obj), Some(more)) = (c.as_object_mut(), extra.as_object()) {
            for (k, v) in more {
                obj.insert(k.clone(), v.clone());
            }
        }
        self.checks.push(c);
        ok
    }

    fn gap(&mut self, floor: usize, why: impl Into<String>) {
        self.gaps.push(json!({"floor": floor, "why": why.into()}));
    }
}

/// The recorded state the replay feeds the engine before each floor.
#[derive(Debug)]
struct Tracked {
    deck: Vec<RunCard>,
    potions: Vec<&'static str>,
    /// Relic ids held.
    relics: Vec<String>,
}

impl Tracked {
    /// Applies one floor's recorded deck, belt and relic changes.
    fn apply(&mut self, stats: &Value, ids_: &mut Ids) {
        for card in stats["cards_transformed"].as_array().into_iter().flatten() {
            remove_card(&mut self.deck, &recorded_card(&card["original_card"]));
            self.deck.push(to_run_card(&card["final_card"], ids_));
        }
        for card in cards(stats, "cards_removed") {
            remove_card(&mut self.deck, &card);
        }
        for card in stats["cards_gained"].as_array().into_iter().flatten() {
            self.deck.push(to_run_card(card, ids_));
        }
        for id in ids(stats, "upgraded_cards") {
            if let Some(c) = self
                .deck
                .iter_mut()
                .filter(|c| c.id == id)
                .min_by_key(|c| c.upgrade_level)
            {
                c.upgrade_level += 1;
            }
        }
        for id in ids(stats, "downgraded_cards") {
            if let Some(c) = self
                .deck
                .iter_mut()
                .filter(|c| c.id == id && c.upgrade_level > 0)
                .max_by_key(|c| c.upgrade_level)
            {
                c.upgrade_level -= 1;
            }
        }
        // A shop lists its potions in `potion_choices` too, so a bought potion is also picked
        // there: count each gained potion once (the multiset union of the two lists).
        let mut gained = picked(stats, "potion_choices");
        let mut unmatched = gained.clone();
        for id in ids(stats, "bought_potions") {
            if let Some(i) = unmatched.iter().position(|p| *p == id) {
                unmatched.remove(i);
            } else {
                gained.push(id);
            }
        }
        for id in gained {
            self.potions.push(ids_.get(&id));
        }
        for id in ids(stats, "potion_used")
            .into_iter()
            .chain(ids(stats, "potion_discarded"))
        {
            if let Some(i) = self.potions.iter().position(|p| *p == id) {
                self.potions.remove(i);
            }
        }
        for id in picked(stats, "relic_choices")
            .into_iter()
            .chain(ids(stats, "bought_relics"))
        {
            if !self.relics.contains(&id) {
                self.relics.push(id);
            }
        }
        for id in ids(stats, "relics_removed") {
            self.relics.retain(|r| *r != id);
        }
    }

    /// Writes the tracked state into `s`, keeping the engine's relic fields where it already has
    /// the relic. Returns the held relics the engine does not port.
    fn sync(&self, s: &mut RunState, stats_before: &Value) -> Vec<String> {
        s.deck.clone_from(&self.deck);
        if let Some(gold) = stats_before["current_gold"].as_i64() {
            s.gold = i32::try_from(gold).unwrap_or(s.gold);
        }
        if let Some(hp) = stats_before["current_hp"].as_i64() {
            s.hp = i32::try_from(hp).unwrap_or(s.hp);
        }
        if let Some(max_hp) = stats_before["max_hp"].as_i64() {
            s.max_hp = i32::try_from(max_hp).unwrap_or(s.max_hp);
        }
        let slots = s.potions.len().max(self.potions.len());
        s.potions = self.potions.iter().map(|&p| Some(p)).collect();
        s.potions.resize(slots, None);
        let mut unported = Vec::new();
        let mut relics = Vec::new();
        for id in &self.relics {
            if let Some(r) = s.relics.iter().find(|r| r.id() == id) {
                relics.push(*r);
            } else if let Some(kind) = RelicKind::from_id(id) {
                relics.push(RunRelic::new(kind));
            } else {
                unported.push(id.clone());
            }
        }
        s.relics = relics;
        unported
    }
}

fn remove_card(deck: &mut Vec<RunCard>, card: &str) {
    let (id, upgraded) = card
        .strip_suffix('+')
        .map_or((card, false), |id| (id, true));
    let at = deck
        .iter()
        .position(|c| c.id == id && (c.upgrade_level > 0) == upgraded)
        .or_else(|| deck.iter().position(|c| c.id == id));
    if let Some(i) = at {
        deck.remove(i);
    }
}

fn to_run_card(card: &Value, ids_: &mut Ids) -> RunCard {
    let mut c = RunCard::new(ids_.get(bare(card["id"].as_str().unwrap_or(""))));
    c.upgrade_level =
        u8::try_from(card["current_upgrade_level"].as_u64().unwrap_or(0)).unwrap_or(0);
    c
}

/// `ActModel.PullNextEvent` on `s` (the engine's `Run::pull_next_event`, which is private): the
/// next allowed, unvisited event of the act's list and the new visit count.
fn next_event(s: &RunState) -> (&'static str, usize) {
    let act = &s.acts[s.act_index];
    let n = act.events.len();
    let mut skipped = 0;
    for _ in 0..n {
        let next = act.events[(act.events_visited + skipped) % n];
        if events::is_allowed(next, s) && !s.visited_event_ids.contains(&next) {
            break;
        }
        skipped += 1;
    }
    let visited = act.events_visited + skipped;
    (
        hooks::modify_next_event(s, act.events[visited % n]),
        visited,
    )
}

/// Runs `generate` on copies of `s` whose `rewards` stream is `k` draws ahead, for the smallest
/// `k` whose result `fits`; that copy becomes `s`. `None` if no `k` up to [`RESYNC_DRAWS`] fits.
fn resync<T>(
    s: &mut RunState,
    generate: impl Fn(&mut RunState) -> T,
    fits: impl Fn(&T) -> bool,
) -> Option<(usize, T)> {
    let mut base = s.clone();
    for k in 0..=RESYNC_DRAWS {
        let mut trial = base.clone();
        let out = generate(&mut trial);
        if fits(&out) {
            *s = trial;
            return Some((k, out));
        }
        base.player_rng[PlayerRngType::Rewards].next_bool();
    }
    None
}

/// `shown` plus each card of `gained` that `predicted` still has room for (as a multiset): what a
/// screen offered when the taken cards are only recorded among the floor's gains.
fn with_taken(shown: &[String], gained: &[String], predicted: &[String]) -> Vec<String> {
    // Upgrades are ignored here: a card taken next to an egg relic is recorded upgraded.
    let base = |c: &String| c.trim_end_matches('+').to_owned();
    let mut room: Vec<String> = predicted.iter().map(base).collect();
    for c in shown {
        if let Some(i) = room.iter().position(|r| *r == base(c)) {
            room.swap_remove(i);
        }
    }
    let mut out = shown.to_vec();
    for c in gained {
        if let Some(i) = room.iter().position(|r| *r == base(c)) {
            room.swap_remove(i);
            out.push(c.clone());
        }
    }
    out
}

/// The first option of a deck selection not picked yet.
fn next_pick(select: &sts2_core::run::CardSelect) -> RunAction {
    RunAction::SelectCard(
        (0..select.options.len())
            .find(|i| !select.picked.contains(i))
            .unwrap_or(0),
    )
}

/// The run's Neow choice in the engine's option order, and the Neow rewards and card offers the
/// engine shows on the way back to the map, checked against the history's first floor.
fn play_neow(run: &mut Run, stats: &Value, report: &mut Report) -> bool {
    let RunDecision::Ancient(room) = run.pending_decision() else {
        report.gap(1, "no Neow");
        return false;
    };
    let options: Vec<String> = room.options.iter().map(|o| o.name().to_owned()).collect();
    let recorded: Vec<String> = stats["ancient_choice"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| c["TextKey"].as_str())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    report.check(
        1,
        "neow_options",
        json!(sorted(options.clone())),
        json!(sorted(recorded)),
        &json!({}),
    );
    let chosen = stats["ancient_choice"]
        .as_array()
        .and_then(|a| a.iter().find(|c| c["was_chosen"].as_bool() == Some(true)))
        .and_then(|c| c["TextKey"].as_str());
    let Some(index) = chosen.and_then(|c| options.iter().position(|o| o == c)) else {
        report.gap(1, format!("Neow choice {chosen:?} not offered"));
        return false;
    };
    let relics_before = run.state().relics.len();
    if let Err(e) = run.apply(RunAction::AncientOption(index)) {
        report.gap(1, format!("Neow option {}: {e:?}", options[index]));
        return false;
    }
    finish_neow(run, stats, report, &options[index], relics_before)
}

/// Answers the prompts the Neow option opened (taking what the player took) until the map, then
/// checks the relics gained and every card and potion offered on the way.
fn finish_neow(
    run: &mut Run,
    stats: &Value,
    report: &mut Report,
    neow_relic: &str,
    relics_before: usize,
) -> bool {
    // Every card and potion Neow's prompts offer; the file merges them into the floor's lists.
    let mut offered = Offer::default();
    for _ in 0..32 {
        let action = match run.pending_decision() {
            RunDecision::Map(_) => {
                let gained: Vec<String> = run.state().relics[relics_before..]
                    .iter()
                    .map(|r| r.id().to_owned())
                    .collect();
                let e = json!({"option": neow_relic});
                report.check(
                    1,
                    "neow_relics",
                    json!(sorted(gained)),
                    json!(sorted(picked(stats, "relic_choices"))),
                    &e,
                );
                // Potions gained at Neow (taken from a reward or procured directly, as
                // `NeowsSacrifice`'s) are in the belt; the file lists them with the untaken ones.
                offered
                    .potions
                    .extend(run.state().potions_held().map(|(_, p)| p.to_owned()));
                // Taken cards appear only in `cards_gained`, untaken ones only in `card_choices`.
                let recorded = with_taken(
                    &card_choices(stats),
                    &cards(stats, "cards_gained"),
                    &offered.cards,
                );
                report.check(1, "neow_offers",
                    json!({"cards": sorted(offered.cards), "potions": sorted(offered.potions)}),
                    json!({"cards": sorted(recorded), "potions": sorted(choices(stats, "potion_choices"))}),
                    &e);
                return true;
            }
            RunDecision::Rewards(rewards) => {
                // Take the relics and potions the player took, then leave. Relics go in the
                // file's order, which is the order they were obtained: `NeowsBones` offers two
                // whose `AfterObtained` may both draw on `rewards` (`ArcaneScroll`, `LargeCapsule`).
                let taken = picked(stats, "relic_choices");
                let potions = picked(stats, "potion_choices");
                let next = taken
                    .iter()
                    .find_map(|t| {
                        rewards
                            .iter()
                            .position(|r| matches!(r, Reward::Relic(id) if id == t))
                    })
                    .or_else(|| {
                        rewards.iter().position(
                            |r| matches!(r, Reward::Potion(id) if potions.iter().any(|t| t == id)),
                        )
                    });
                if let Some(i) = next {
                    RunAction::ClaimReward(i)
                } else {
                    let offer = Offer::of(rewards);
                    offered.cards.extend(offer.cards);
                    offered.potions.extend(offer.potions);
                    RunAction::Proceed
                }
            }
            RunDecision::CardSelect(select) if select.purpose.picks_offered() => {
                offered.cards.extend(select.offered.iter().map(engine_card));
                if select.min() == 0 {
                    RunAction::CancelSelect
                } else {
                    next_pick(select)
                }
            }
            RunDecision::CardSelect(select) => {
                if select.picked.len() >= select.min() && select.min() < select.count {
                    RunAction::ConfirmSelect
                } else {
                    next_pick(select)
                }
            }
            RunDecision::Bundles(_) => RunAction::SelectBundle(0),
            _ => {
                report.gap(1, format!("Neow option {neow_relic}: unexpected prompt"));
                return false;
            }
        };
        if let Err(e) = run.apply(action) {
            report.gap(1, format!("Neow option {neow_relic}: {e:?}"));
            return false;
        }
    }
    report.gap(1, format!("Neow option {neow_relic}: prompts did not end"));
    false
}

/// The replay's state between floors.
struct Replay {
    /// The engine state the generators run on.
    s: RunState,
    tracked: Tracked,
    ids: Ids,
    report: Report,
    /// Whether the `rewards` stream is known to be where the game's was.
    synced: bool,
    /// Whether a resync has happened (later checks are flagged).
    resynced: bool,
    /// The held relics the engine does not port, this floor.
    unported: Vec<String>,
    /// A chest relic the floor pulled but the file does not record (not taken).
    skipped_chest: Option<&'static str>,
}

impl Replay {
    /// The extra fields of a check: `resynced` (when `chained`) and `unported_relics`.
    fn extra(&self, chained: bool) -> Value {
        let mut e = json!({"act": self.s.act_index + 1});
        if chained && self.resynced {
            e["resynced"] = json!(true);
        }
        if !self.unported.is_empty() {
            e["unported_relics"] = json!(self.unported);
        }
        e
    }

    /// `generate` on the state if in sync, else on the first resynced copy that `fits`.
    fn generate<T>(
        &mut self,
        floor: usize,
        generate: impl Fn(&mut RunState) -> T,
        fits: impl Fn(&T) -> bool,
    ) -> Option<T> {
        if self.synced {
            return Some(generate(&mut self.s));
        }
        let (k, out) = resync(&mut self.s, generate, fits)?;
        self.report.gap(floor, format!("resynced after {k} draws"));
        self.synced = true;
        self.resynced = true;
        Some(out)
    }

    /// After a chained check fails, later floors are checked after a resync, not as a cascade.
    fn broke(&mut self, floor: usize) {
        self.synced = false;
        self.report.gap(floor, "mismatch");
    }

    fn combat(&mut self, floor: usize, room_type: RoomType, entry: &Value) {
        let stats = &entry["player_stats"][0];
        let encounter = entry["rooms"][0]["model_id"].as_str().map_or("", bare);
        let in_deck = |id: &str| self.tracked.deck.iter().any(|c| c.id == id);
        // `TheHunt` (a fatal hit adds a 3-card `ForRoom` reward, `TheHunt.cs:47`) shows as a
        // second card reward; the fight that played it is not recorded otherwise.
        let hunt = in_deck("THE_HUNT") && card_choices(stats).len() >= 6;
        // `HeistPower` (`GremlinMercNormal`'s fat gremlin dying) and `RoyaltiesPower` add a
        // `GoldReward`, one `rewards` draw after the room's own, decided inside the fight.
        let fight_gold = encounter == "GREMLIN_MERC_NORMAL" || in_deck("ROYALTIES");
        let recorded = Offer {
            cards: sorted(card_choices(stats)),
            potions: sorted(choices(stats, "potion_choices")),
            relics: sorted(choices(stats, "relic_choices")),
            gold: i32::try_from(stats["gold_gained"].as_i64().unwrap_or(0)).unwrap_or(0),
        };
        let fits = |o: &Offer| o.cards == recorded.cards && o.potions == recorded.potions;
        let roll = |s: &mut RunState, gold: GoldProportion| {
            let extra = if hunt {
                let odds = CardRarityOddsType::for_room(room_type);
                vec![PendingReward::Card(CardCreation::for_room(odds), 3)]
            } else {
                Vec::new()
            };
            let rewards = rewards::generate_combat_rewards(s, room_type, gold, extra);
            let mut o = Offer::of(&rewards);
            o.cards.sort();
            o.potions.sort();
            o.relics.sort();
            o
        };
        // `GoldProportion` is not recorded: 0 (every gold holder escaped, as `GremlinMercNormal`'s
        // thief can) skips the gold draw, any other value only scales the amount.
        let generate = |s: &mut RunState| {
            let mut full = s.clone();
            let o = roll(&mut full, GoldProportion::ONE);
            if !fits(&o) && room_type == RoomType::Monster {
                let mut none = s.clone();
                let o0 = roll(&mut none, GoldProportion::ZERO);
                if fits(&o0) {
                    *s = none;
                    return (o0, true);
                }
            }
            *s = full;
            (o, false)
        };
        let Some((p, no_gold)) = self.generate(floor, generate, |(o, _)| fits(o)) else {
            return;
        };
        let e = self.extra(true);
        let r = &mut self.report;
        let mut ok = r.check(
            floor,
            "card_reward",
            json!(p.cards),
            json!(recorded.cards),
            &e,
        );
        ok &= r.check(
            floor,
            "potion_reward",
            json!(p.potions),
            json!(recorded.potions),
            &e,
        );
        if room_type == RoomType::Elite {
            ok &= r.check(
                floor,
                "relic_reward",
                json!(p.relics),
                json!(recorded.relics),
                &e,
            );
        }
        if !no_gold {
            r.check(
                floor,
                "combat_gold",
                json!(p.gold),
                json!(recorded.gold),
                &e,
            );
        }
        if !ok {
            self.broke(floor);
        } else if fight_gold {
            self.synced = false;
            self.report.gap(floor, "fight-added gold reward");
        }
    }

    fn treasure(&mut self, floor: usize, stats: &Value) {
        hooks::after_room_entered(&mut self.s, RoomType::Treasure);
        if !hooks::should_generate_treasure(&self.s) {
            return;
        }
        // The relic comes from its own stream and the bag, so it is checked even out of sync.
        let relic = rewards::roll_treasure_relic(&mut self.s);
        let e = self.extra(false);
        let recorded = choices(stats, "relic_choices");
        // `BeginRelicPicking` pulls from the shared bag on entering, so a relic left in the
        // chest is gone from it too (`TreasureRoomRelicSynchronizer.cs`).
        if recorded.is_empty() {
            self.skipped_chest = Some(relic);
        }
        self.report
            .check(floor, "treasure_relic", json!([relic]), json!(recorded), &e);
        let gold = self.s.player_rng[PlayerRngType::Rewards].next_int_range(42, 53);
        if self.synced {
            // `PlayerCmd.GainGold` runs `ModifyGoldGained` before the file records the amount.
            let base = Decimal::from(rewards::poverty_gold(gold, self.s.ascension));
            let gained = hooks::modify_gold_gained(&self.s, base)
                .0
                .to_int_truncating();
            let e = self.extra(true);
            self.report.check(
                floor,
                "treasure_gold",
                json!(gained),
                json!(stats["gold_gained"]),
                &e,
            );
        }
    }

    fn shop(&mut self, floor: usize, stats: &Value) {
        // Bought cards are only in `cards_gained`, which also has cards from other sources: the
        // recorded stock is `card_choices` plus the gained cards the prediction has room for.
        let unbought = card_choices(stats);
        let gained = cards(stats, "cards_gained");
        let relics = sorted(choices(stats, "relic_choices"));
        let potions = sorted(choices(stats, "potion_choices"));
        let generate = |s: &mut RunState| {
            let shop = Shop::generate(s);
            (
                sorted(
                    shop.cards
                        .iter()
                        .filter_map(|c| c.card)
                        .map(|c| engine_card(&c))
                        .collect(),
                ),
                sorted(
                    shop.relics
                        .iter()
                        .filter_map(|r| r.id)
                        .map(str::to_owned)
                        .collect(),
                ),
                sorted(
                    shop.potions
                        .iter()
                        .filter_map(|p| p.id)
                        .map(str::to_owned)
                        .collect(),
                ),
            )
        };
        let fits = |o: &(Vec<String>, Vec<String>, Vec<String>)| {
            o.0 == sorted(with_taken(&unbought, &gained, &o.0)) && o.1 == relics
        };
        if let Some((c, r, p)) = self.generate(floor, generate, fits) {
            let e = self.extra(true);
            let stock = sorted(with_taken(&unbought, &gained, &c));
            let ok = self
                .report
                .check(floor, "shop_cards", json!(c), json!(stock), &e)
                & self
                    .report
                    .check(floor, "shop_relics", json!(r), json!(relics), &e);
            self.report
                .check(floor, "shop_potions", json!(p), json!(potions), &e);
            if !ok {
                self.broke(floor);
            }
        }
        hooks::after_room_entered(&mut self.s, RoomType::Shop);
        if !cards(stats, "cards_removed").is_empty() {
            self.s.card_shop_removals_used += 1;
        }
    }

    fn rest_site(&mut self, floor: usize, stats: &Value) {
        for choice in ids(stats, "rest_site_choices") {
            match choice.as_str() {
                "DIG" if self.synced => {
                    let relic = rewards::pull_relic_from_front(&mut self.s);
                    let e = self.extra(true);
                    self.report.check(
                        floor,
                        "dig_relic",
                        json!([relic]),
                        json!(choices(stats, "relic_choices")),
                        &e,
                    );
                }
                "HEAL" if self.synced => {
                    if !self.s.rest_site_heal(false).is_empty() {
                        self.synced = false;
                        self.report.gap(floor, "rest-site heal reward");
                    }
                }
                "SMITH" | "LIFT" | "HATCH" | "DIG" | "HEAL" => {}
                other => {
                    if self.synced {
                        self.report.gap(floor, format!("rest option {other}"));
                    }
                    self.synced = false;
                }
            }
        }
    }

    fn event(&mut self, floor: usize, entry: &Value) {
        let recorded = entry["rooms"][0]["model_id"].as_str().map_or("", bare);
        // `CreateRoom` pulls the event before `AppendToMapPointHistory`: `TotalFloor` excludes this floor.
        self.s.history.truncate(floor - 1);
        if entry["map_point_type"] == "ancient" && self.s.act_index > 0 {
            // `MarkRoomVisited(Event)` after the act's ancient (Neow's is counted by the `Run`).
            self.s.acts[self.s.act_index].events_visited += 1;
        }
        if entry["map_point_type"] == "unknown" {
            let (predicted, visited) = next_event(&self.s);
            let e = self.extra(false);
            self.report
                .check(floor, "event", json!(predicted), json!(recorded), &e);
            // Continue from the recorded event, as the game did, then `MarkRoomVisited`.
            let act = &mut self.s.acts[self.s.act_index];
            let n = act.events.len();
            // A deck `LanternKey` swaps the pulled event for `WarHistorianRepy`
            // (`ModifyNextEvent`): the list still moves past the event it replaced, and the
            // swapped-in one is what `AddVisitedEvent` records.
            let swapped = recorded == "WAR_HISTORIAN_REPY";
            let at = if swapped {
                visited
            } else {
                (0..n)
                    .map(|d| act.events_visited + d)
                    .find(|&v| act.events[v % n] == recorded)
                    .unwrap_or(visited)
            };
            let id = if swapped {
                act.events.iter().copied().find(|e| *e == recorded)
            } else {
                Some(act.events[at % n])
            };
            act.events_visited = at + 1;
            self.s.visited_event_ids.extend(id);
        }
        // Events draw on `rewards` in ways not replayed here.
        if self.synced {
            self.report.gap(floor, format!("event {recorded}"));
        }
        self.synced = false;
    }

    /// One floor after Neow: the recorded state in, the room's checks, the floor's changes out.
    fn floor(&mut self, i: usize, entry: &Value, before: &Value) {
        let floor = i + 1;
        let stats = &entry["player_stats"][0];
        self.unported = self.tracked.sync(&mut self.s, before);
        crate::capacity::observe_run(&self.s, || format!("floor {floor}"));
        // `TotalFloor` counts this floor too, Neow included (`AppendToMapPointHistory` runs before
        // the room's rewards and `IsBeforeAct3TreasureChest` reads it).
        self.s.history.truncate(floor);
        self.s.history.resize(
            floor,
            MapPointHistoryEntry {
                point_type: MapPointType::Monster,
                room_type: Some(RoomType::Monster),
                model: None,
            },
        );
        // The generators below draw the engine's own relics from the bags; the game's bags lost
        // the recorded ones instead (`lose_offered`), so they are put back first.
        let bags = (
            self.s.shared_relic_bag.clone(),
            self.s.player_relic_bag.clone(),
        );
        match entry["rooms"][0]["room_type"].as_str().unwrap_or("") {
            "monster" => self.combat(floor, RoomType::Monster, entry),
            "elite" => self.combat(floor, RoomType::Elite, entry),
            "boss" => self.combat(floor, RoomType::Boss, entry),
            "treasure" => self.treasure(floor, stats),
            "shop" => self.shop(floor, stats),
            "rest_site" => self.rest_site(floor, stats),
            "event" => self.event(floor, entry),
            _ => {}
        }
        (self.s.shared_relic_bag, self.s.player_relic_bag) = bags;
        if let Some(relic) = self.skipped_chest.take() {
            self.s.shared_relic_bag.remove(relic);
        }
        self.tracked.apply(stats, &mut self.ids);
        self.lose_offered(stats);
    }

    /// The bags lose every relic the floor offered or gave, and the ones an event pulled
    /// without recording them: `hidden_relic_pulls`, which `tools/replay_history.py` adds from
    /// the event's title variables (`RelicTrader`'s three `NewRelics`, `WelcomeToWongos`'
    /// `FeaturedItem`, each a `PullNextRelicFromFront`).
    fn lose_offered(&mut self, stats: &Value) {
        for id in choices(stats, "relic_choices")
            .into_iter()
            .chain(ids(stats, "bought_relics"))
            .chain(ids(stats, "hidden_relic_pulls"))
        {
            self.s.shared_relic_bag.remove(&id);
            self.s.player_relic_bag.remove(&id);
        }
    }
}

/// Replays the floors of `run` (a `.run` file's JSON) for `character` on `seed`: Neow on
/// the engine's [`Run`], then each floor's generators on a copy of its state.
///
/// # Errors
/// If the run cannot be created.
pub fn replay(
    character: Character,
    seed: &str,
    ascension: u8,
    act1: Act1,
    run_json: &Value,
) -> Result<Value> {
    let mut run = Run::new(character, seed, ascension, UnlockState::all(), act1)?;
    // Every act's floors in order, with the act index (Neow is the first).
    let floors: Vec<(usize, &Value)> = run_json["map_point_history"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .flat_map(|(act, floors)| {
            floors
                .as_array()
                .into_iter()
                .flatten()
                .map(move |f| (act, f))
        })
        .collect();
    let Some(&(_, first)) = floors.first() else {
        return Ok(json!({"checks": [], "gaps": []}));
    };
    let neow_stats = &first["player_stats"][0];
    let tracked = Tracked {
        deck: run.state().deck.clone(),
        potions: Vec::new(),
        relics: run
            .state()
            .relics
            .iter()
            .map(|r| r.id().to_owned())
            .collect(),
    };
    let mut report = Report::default();
    let synced = play_neow(&mut run, neow_stats, &mut report);
    let mut replay = Replay {
        s: run.state().clone(),
        tracked,
        ids: Ids::default(),
        report,
        synced,
        resynced: false,
        unported: Vec::new(),
        skipped_chest: None,
    };
    replay.tracked.apply(neow_stats, &mut replay.ids);
    // Also when Neow stopped part way (an unported relic among `NeowsBones`' gifts).
    replay.lose_offered(neow_stats);
    for (i, pair) in floors.windows(2).enumerate() {
        let (act, entry) = pair[1];
        if act >= replay.s.acts.len() {
            break;
        }
        replay.s.act_index = act;
        replay.floor(i + 1, entry, &pair[0].1["player_stats"][0]);
    }
    Ok(json!({"checks": replay.report.checks, "gaps": replay.report.gaps}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shop lists a bought potion in both `bought_potions` and the picked `potion_choices`; the
    /// belt gains it once (community run `MFNTU0QSTUTY` f31: a doubled `CURE_ALL` outlived the f33
    /// fight, so the f37 event's `IsAllowed` checks saw a potion the belt no longer had).
    #[test]
    fn shop_bought_potion_counts_once() {
        let mut tracked = Tracked {
            deck: Vec::new(),
            potions: Vec::new(),
            relics: Vec::new(),
        };
        let stats = json!({
            "bought_potions": ["POTION.CURE_ALL"],
            "potion_choices": [
                {"choice": "POTION.CURE_ALL", "was_picked": true},
                {"choice": "POTION.STAR_POTION", "was_picked": false},
            ],
        });
        tracked.apply(&stats, &mut Ids::default());
        assert_eq!(tracked.potions, ["CURE_ALL"]);

        // A bought potion the file does not also list as picked still counts.
        let stats = json!({"bought_potions": ["POTION.FIRE_POTION", "POTION.CURE_ALL"]});
        tracked.apply(&stats, &mut Ids::default());
        assert_eq!(tracked.potions, ["CURE_ALL", "FIRE_POTION", "CURE_ALL"]);
    }
}
