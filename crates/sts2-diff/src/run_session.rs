//! Plays a whole [`Run`] in the referee's run-mode terms: referee actions out, `{step, action,
//! decision, state, run}` trace lines in the referee's schema, restricted to modeled fields.
//!
//! sts2-cli decides some things for the player; the session scripts the same defaults on the
//! engine after every action (see `referee/README.md`): gold, relic and potion rewards are
//! claimed in one pass in reward order (a potion that does not fit stays, also once a later relic
//! frees a slot), the rewards screen is left once the first card reward is answered (a second
//! one is dropped), and a treasure room's relic is taken. A finished event, or a rest site once an option is used, is left for the map at once.

use serde_json::{Value, json};
use sts2_core::combat::{Action, CombatState, DecisionRequest};
use sts2_core::content::events::EventKind;
use sts2_core::rng::{PlayerRngType, RunRngType};
use sts2_core::run::{
    Act1, CardAlternative, CardSelect, EventRoom, MapCoord, Reward, Room, Run, RunAction, RunCard,
    RunDecision, RunError, RunState, SelectPurpose, Shop, THE_ARCHITECT, UnlockState, shop,
};
use sts2_core::{Character, Combat};

use crate::session::{
    REFEREE_BROKEN_CARDS, decimal_json, referee_broken_play, referee_order, state_json, to_referee,
};
use sts2_core::content::cards;

use crate::{Result, fields, ids, loadout, map};

/// Event options (event id, option tag) that throw in the referee before doing their work:
/// `JungleMazeAdventure`'s `SafetyInNumbers` calls `NDebugAudioManager.Instance.Play`, and
/// `Instance` is `NGame.Instance?.DebugAudio`, null in headless. `DenseVegetation`'s `Rest` heals
/// (like the engine), then throws at its `NDebugAudioManager.Instance.Play("sleep.tres")`, so the
/// event stays on its first page and its Fight page is unreachable. Both of `CrystalSphere`'s
/// options open its minigame screen, which headless sts2-cli awaits forever (the event stays on
/// its first page). `Trial`'s `DoubleDown` opens `NAbandonRunConfirmPopup`, which headless sts2-cli
/// never confirms: the option stays and the referee marks it chosen (`WasChosen`), the engine's
/// page does not.
const REFEREE_BROKEN_OPTIONS: [(&str, &str); 5] = [
    ("JUNGLE_MAZE_ADVENTURE", "JOIN_FORCES"),
    ("DENSE_VEGETATION", "REST"),
    ("CRYSTAL_SPHERE", "UNCOVER_FUTURE"),
    ("CRYSTAL_SPHERE", "PAYMENT_PLAN"),
    ("TRIAL", "DOUBLE_DOWN"),
];

/// Events without options (a custom layout) that sts2-cli leaves at once for the map.
const REFEREE_SKIPPED_EVENTS: [&str; 1] = ["FAKE_MERCHANT"];

/// Relics that react to gold gained. sts2-cli takes a treasure room's relic before the chest's
/// gold (`TreasureState`), while the game gives the gold first (`NTreasureRoom.OpenChest`:
/// `DoNormalRewards`, then the relic picking), so a treasure room holding one of these has no
/// oracle.
const GOLD_RELICS: [&str; 2] = ["BOWLER_HAT", "DRAGON_FRUIT"];

/// The relics that upgrade cards of one type as they are offered.
const EGG_RELICS: [&str; 3] = ["FROZEN_EGG", "TOXIC_EGG", "MOLTEN_EGG"];

/// Relics the referee cannot check.
///
/// sts2-cli builds a fight's rewards itself (`RewardsSet.GenerateWithoutOffering`) and never calls
/// `Hook.BeforeCombatRewardOffered`, which is how `LastingCandy` counts the card rewards it has
/// seen, so it never adds its card there. It also answers a card reward without
/// `CardReward.OnSelect`, which is what unsubscribes the reward from `Player.RelicObtained`, so a
/// relic obtained later runs `TryModifyCardRewardOptionsLate` on every earlier card reward:
/// `WingCharm` draws `Niche` once for each of them.
const NO_ORACLE_RELICS: [&str; 2] = ["LASTING_CANDY", "WING_CHARM"];

/// The entities, options and plays the differential harness cannot check against the referee.
/// `sts2-diff oracle-gaps` prints these; they are the honest boundary of a parity claim.
#[must_use]
pub fn oracle_gaps() -> Vec<String> {
    let mut gaps = Vec::new();
    for (event, tag) in REFEREE_BROKEN_OPTIONS {
        gaps.push(format!("option {event} {tag}"));
    }
    for event in REFEREE_SKIPPED_EVENTS {
        gaps.push(format!("event {event}"));
    }
    for relic in GOLD_RELICS {
        gaps.push(format!("treasure {relic} (relic before gold)"));
    }
    for relic in NO_ORACLE_RELICS {
        gaps.push(format!("relic {relic}"));
    }
    gaps.push("play VOID_FORM whose end of turn wins the fight".to_owned());
    gaps.push("play VOID_FORM whose end of turn opens a choice".to_owned());
    gaps.push("combat loadout BIIIG_HUG (card-removal prompt)".to_owned());
    gaps.push("monster SOUL_NEXUS (death handler null room)".to_owned());
    gaps
}

/// A run plus its seed and step count.
#[derive(Clone, Debug)]
pub struct RunSession {
    seed: String,
    ascension: u8,
    run: Run,
    step: u64,
    /// Event rooms of ported events entered so far (Neow not counted).
    events_entered: usize,
    /// Shops entered so far.
    shops_entered: usize,
    /// Shop purchases so far: cards, relics, potions, card removals.
    purchases: [usize; 4],
    /// The highest floor of the run reached (`TotalFloor`; in Act 1 the act floor).
    max_floor: usize,
    /// The act index of the last trace line: a line in a new act carries its map and room lists.
    act_index: usize,
}

impl RunSession {
    /// A fresh run of `character` on `seed` at `ascension` with the referee's unlock state, in
    /// test mode (sts2-cli sets `TestMode.IsOn`), or with `real_game` out of it, which the referee
    /// reaches with `"real_shop_prices"`, `"real_cauldron"` and `"real_calling_bell"` (`referee/patcher`). `act1` is forced
    /// (the referee never rolls it: [`act1_of`]).
    ///
    /// # Errors
    /// If the run cannot be created, or `act1` is [`Act1::Random`].
    pub fn new(
        character: Character,
        seed: &str,
        ascension: u8,
        real_game: bool,
        act1: Act1,
    ) -> Result<Self> {
        if act1 == Act1::Random {
            return Err("the referee cannot roll Act 1: force overgrowth or underdocks".into());
        }
        let run = Run::new(character, seed, ascension, UnlockState::all(), act1)?.with_events();
        let run = if real_game { run } else { run.in_test_mode() };
        Ok(Self {
            seed: seed.to_owned(),
            ascension,
            max_floor: run.state().total_floor(),
            act_index: run.state().act_index,
            run,
            step: 0,
            events_entered: 0,
            shops_entered: 0,
            purchases: [0; 4],
        })
    }

    /// The run.
    #[must_use]
    pub const fn run(&self) -> &Run {
        &self.run
    }

    /// The number of event rooms (ported events, Neow not counted) entered so far.
    #[must_use]
    pub const fn events_entered(&self) -> usize {
        self.events_entered
    }

    /// The highest floor of the run reached (`TotalFloor`, which in Act 1 is the act floor).
    #[must_use]
    pub const fn final_floor(&self) -> usize {
        self.max_floor
    }

    /// The number of shops entered so far.
    #[must_use]
    pub const fn shops_entered(&self) -> usize {
        self.shops_entered
    }

    /// Shop purchases so far: cards, relics, potions, card removals.
    #[must_use]
    pub const fn purchases(&self) -> [usize; 4] {
        self.purchases
    }

    /// The trace line for step 0, with the map and the pre-rolled room lists.
    #[must_use]
    pub fn start_line(&self) -> Value {
        json!({
            "step": 0,
            "action": {
                "action": "start",
                "seed": self.seed,
                "character": self.run.state().character.name(),
                "ascension": self.ascension,
                "mode": "run",
            },
            "decision": decision_name(&self.run),
            "state": json!({}),
            "run": act_start_run_json(&self.run, None),
        })
    }

    /// Gives the run a loadout (see [`crate::loadout::apply_run`]) while it waits on the map,
    /// lets sts2-cli claim the rewards a relic offered, and returns the referee action
    /// (`run_loadout`) and the trace line.
    ///
    /// # Errors
    /// If the loadout is malformed, the run is not on the map, or a relic the referee cannot
    /// check ([`NO_ORACLE_RELICS`]) is among the new ones.
    pub fn apply_loadout(&mut self, loadout: &Value) -> Result<(Value, Value)> {
        loadout::apply_run(&mut self.run, loadout)?;
        settle(&mut self.run)?;
        if let Some(relic) = self
            .run
            .state()
            .relics
            .iter()
            .find(|r| NO_ORACLE_RELICS.contains(&r.id()))
        {
            return Err(format!("referee relic {}", relic.id()).into());
        }
        self.step += 1;
        let action = json!({"action": "run_loadout", "args": loadout});
        let line = json!({
            "step": self.step,
            "action": action,
            "decision": decision_name(&self.run),
            "state": json!({}),
            "run": self.run_line_json(None),
        });
        Ok((action, line))
    }

    /// Forces the event `id` while the run waits on the map ([`Run::debug_enter_event`]), as
    /// the referee's `enter_event` does, and returns that action and the trace line.
    ///
    /// # Errors
    /// If the event is not ported or the run is not on the map.
    pub fn enter_event(&mut self, id: &str) -> Result<(Value, Value)> {
        let kind = EventKind::from_id(id).ok_or_else(|| format!("event {id} is not ported"))?;
        self.run.debug_enter_event(kind)?;
        self.events_entered += 1;
        self.step += 1;
        let action = json!({"action": "enter_event", "args": {"event": id}});
        let line = json!({
            "step": self.step,
            "action": action,
            "decision": decision_name(&self.run),
            "state": json!({}),
            "run": self.run_line_json(None),
        });
        Ok((action, line))
    }

    /// The run object of the next trace line: with the act's map and room lists on the first line
    /// of a new act (the referee dumps them there).
    fn run_line_json(&mut self, ended: Option<&CombatState>) -> Value {
        let act_index = self.run.state().act_index;
        if std::mem::replace(&mut self.act_index, act_index) == act_index {
            run_json(&self.run, ended)
        } else {
            act_start_run_json(&self.run, ended)
        }
    }

    /// Whether `action` is accepted, can be sent to the referee, and leads somewhere the engine
    /// models (with the referee's defaults applied): no `Unsupported*` error, no stub room, no
    /// fight stopped at unported content and no unported card in the deck.
    ///
    /// # Errors
    /// The reason it is not, as text (e.g. `encounter BYGONE_EFFIGY_ELITE`).
    pub fn check(&self, action: &RunAction) -> std::result::Result<(), String> {
        referee_action(&self.run, action).map_err(|e| e.to_string())?;
        if let (RunAction::EventOption(i), RunDecision::Event(room)) =
            (action, self.run.pending_decision())
        {
            let id = room.kind.id();
            let tag = room.options.get(*i).and_then(|o| o.action).map(|a| a.tag);
            if tag.is_some_and(|tag| REFEREE_BROKEN_OPTIONS.contains(&(id, tag))) {
                return Err(format!("referee {id} {}", tag.unwrap_or_default()));
            }
        }
        if let (
            RunAction::PickCard { reward: i, .. }
            | RunAction::SkipReward(i)
            | RunAction::CardAlternative { reward: i, .. },
            RunDecision::Rewards(rewards),
        ) = (action, self.run.pending_decision())
        {
            // sts2-cli answers the first card reward only; skipping one while more follow
            // outside a fight is not understood either (`Orrery`).
            let is_card = |r: &Reward| matches!(r, Reward::Card(_));
            let first = rewards.iter().position(is_card);
            if rewards.get(*i).is_some_and(is_card) {
                if first != Some(*i) {
                    return Err("referee card reward order".to_owned());
                }
                let more = rewards.iter().filter(|r| is_card(r)).count() > 1;
                if matches!(action, RunAction::SkipReward(_))
                    && more
                    && !matches!(self.run.state().room, Room::Combat { .. })
                {
                    return Err("referee card reward skip".to_owned());
                }
            }
        }
        if let (RunAction::Combat(a), Some(combat)) = (action, self.run.combat())
            && let Some(kind) = referee_broken_play(combat, a)
        {
            return Err(format!("referee card {}", kind.entry()));
        }
        if matches!(action, RunAction::ChooseNode(_)) {
            let mut run = self.run.clone();
            if run.apply(action.clone()).is_ok()
                && let RunDecision::Treasure(Some(id)) = run.pending_decision()
                && GOLD_RELICS.contains(&id)
            {
                return Err(format!("referee treasure {id}"));
            }
        }
        let mut run = self.run.clone();
        advance(&mut run, action).map_err(|e| match e {
            RunError::UnsupportedEncounter(id) => format!("encounter {id}"),
            RunError::UnsupportedAncientOption(id) => format!("neow {id}"),
            RunError::UnsupportedRelic(id) => format!("relic {id}"),
            RunError::UnsupportedCard(id) => format!("card {id}"),
            RunError::UnsupportedPotion(id) => format!("potion {id}"),
            e => e.to_string(),
        })?;
        if let Some(relic) = run
            .state()
            .relics
            .iter()
            .find(|r| NO_ORACLE_RELICS.contains(&r.id()))
        {
            return Err(format!("referee relic {}", relic.id()));
        }
        if let Some(card) = run.state().deck.iter().find(|c| c.to_combat().is_none()) {
            return Err(format!("card {}", card.id));
        }
        if let Some(card) = run.state().deck.iter().find(|c| {
            c.to_combat()
                .is_some_and(|c| REFEREE_BROKEN_CARDS.contains(&c.kind))
        }) {
            return Err(format!("referee card {}", card.id));
        }
        if let Some(card) = egg_shop_card(&run) {
            return Err(format!("shop card {card} with an egg"));
        }
        match run.pending_decision() {
            // A fight that met unported content (a generated card).
            RunDecision::Stopped(unsupported) => Err(unsupported.to_string()),
            // `FakeMerchant` has a custom layout and no `EventOption`s; sts2-cli auto-skips such
            // an event to the map (`EventChoiceState`), so its screen has no oracle.
            RunDecision::Event(room) if REFEREE_SKIPPED_EVENTS.contains(&room.kind.id()) => {
                Err(format!("referee event {}", room.kind.id()))
            }
            RunDecision::Unsupported(_) if let Room::Event { id } = run.state().room => {
                Err(format!("event {id}"))
            }
            RunDecision::Unsupported(point) => Err(format!(
                "room {}",
                run.state()
                    .room_type()
                    .map_or_else(|| point.name(), |r| r.name())
            )),
            _ => Ok(()),
        }
    }

    /// Whether `action` passes [`RunSession::check`] and leaves at least one action that does
    /// (or ends the run): a one-step deeper check, for choices whose prompt may offer nothing
    /// modeled.
    #[must_use]
    pub fn leads_somewhere(&self, action: &RunAction) -> bool {
        if self.check(action).is_err() {
            return false;
        }
        let mut next = self.clone();
        if next.apply(action).is_err() {
            return false;
        }
        let run = next.run();
        matches!(
            run.pending_decision(),
            RunDecision::GameOver | RunDecision::Victory | RunDecision::Combat(_)
        ) || run.legal_actions().iter().any(|a| next.check(a).is_ok())
    }

    /// Applies an engine action (and the referee's defaults after it) and returns the
    /// referee-format action and the trace line; `None` for a card pick that does not finish
    /// its selection yet (the referee takes all picks in one `select_cards`).
    ///
    /// # Errors
    /// If the run rejects the action or it has no referee equivalent.
    pub fn apply(&mut self, action: &RunAction) -> Result<Option<(Value, Value)>> {
        let referee_action = referee_action(&self.run, action)?;
        let Advance { ended, .. } = advance(&mut self.run, action)?;
        self.max_floor = self.max_floor.max(self.run.state().total_floor());
        match action {
            RunAction::BuyCard(_) => self.purchases[0] += 1,
            RunAction::BuyRelic(_) => self.purchases[1] += 1,
            RunAction::BuyPotion(_) => self.purchases[2] += 1,
            RunAction::RemoveCard => self.purchases[3] += 1,
            _ => {}
        }
        if matches!(action, RunAction::ChooseNode(_)) {
            match self.run.pending_decision() {
                RunDecision::Event(_) => self.events_entered += 1,
                RunDecision::Shop(_) => self.shops_entered += 1,
                _ => {}
            }
        }
        let Some(referee_action) = referee_action else {
            return Ok(None);
        };
        self.step += 1;
        // A fight the action ended is gone from the run; its final state is dumped like the
        // referee does, also when it ended as its room was entered (`Advance::ended`).
        let mut state = self
            .run
            .combat()
            .or(ended.as_deref())
            .map_or_else(|| json!({}), combat_state_json);
        // The fight is over but the dump still shows the player, whom sts2-cli's reward claims
        // changed after the last play (`DragonFruit`: gold gained is +1 max HP and HP; a relic
        // reward joins `Relics`).
        // A fight that just ended resolves the card ids its relics still hold.
        let ended = ended.as_deref().map(Combat::state);
        if ended.is_some() && self.run.pending_decision() != RunDecision::GameOver {
            state["player"]["hp"] = json!(self.run.state().hp);
            state["player"]["max_hp"] = json!(self.run.state().max_hp);
            state["player"]["relics"] = json!(
                self.run
                    .state()
                    .relics
                    .iter()
                    .map(|r| fields::relic_json(ended, r.kind, &r.data))
                    .collect::<Vec<_>>()
            );
        }
        // A boss kill can leave the combat room at once (A10's first boss back to the map, the
        // last boss into TheArchitect's room): the referee then dumps no fight, only the flags
        // of the one that ended (PX16).
        if ended.is_some() && !matches!(self.run.state().room, Room::Combat { .. }) {
            state = json!({"combat_over": true, "player_dead": false});
        }
        let line = json!({
            "step": self.step,
            "action": referee_action,
            "decision": decision_name(&self.run),
            "state": state,
            "run": self.run_line_json(ended),
        });
        Ok(Some((referee_action, line)))
    }
}

/// An unported card in the shop while an egg relic is held: an egg upgrades the shop's cards of
/// its type, and whether an unported card (`Splash`) can be upgraded is not known.
fn egg_shop_card(run: &Run) -> Option<&'static str> {
    let RunDecision::Shop(shop) = run.pending_decision() else {
        return None;
    };
    if !run
        .state()
        .relics
        .iter()
        .any(|r| EGG_RELICS.contains(&r.id()))
    {
        return None;
    }
    shop.cards
        .iter()
        .filter_map(|c| c.card)
        .find(|c| c.to_combat().is_none())
        .map(|c| c.id)
}

/// Applies `action`, then what sts2-cli does on its own before the next prompt.
fn advance(run: &mut Run, action: &RunAction) -> std::result::Result<Advance, RunError> {
    let mut advance = Advance::default();
    let answers_card_reward = match (action, run.pending_decision()) {
        (RunAction::PickCard { .. }, _) => true,
        // A reroll keeps the reward on the screen; a relic's alternative (the sacrifice) completes it.
        (
            RunAction::CardAlternative {
                reward,
                alternative,
            },
            _,
        ) => matches!(
            run.card_reward_alternatives(*reward).get(*alternative),
            Some(CardAlternative::Relic { .. })
        ),
        (RunAction::SkipReward(i), RunDecision::Rewards(rewards)) => {
            matches!(rewards.get(*i), Some(Reward::Card(_)))
        }
        _ => false,
    };
    run.apply(action.clone())?;
    advance.ended = run.ended_combat().cloned().map(Box::new);
    // sts2-cli presents one card reward of a fight and leaves the screen once it is answered,
    // dropping any other card reward (`PrayerWheel`, `WhiteStar`). Outside a fight it presents
    // them one after the other (`Orrery`).
    if answers_card_reward
        && matches!(run.state().room, Room::Combat { .. })
        && matches!(run.pending_decision(), RunDecision::Rewards(_))
    {
        run.apply(RunAction::Proceed)?;
    }
    settle(run)?;
    Ok(advance)
}

/// What sts2-cli does by itself after an action (or a loadout relic) until the next prompt: it
/// claims the rewards, takes the treasure and leaves a finished rest site.
fn settle(run: &mut Run) -> std::result::Result<(), RunError> {
    let mut passes = Vec::new();
    loop {
        let next = match run.pending_decision() {
            RunDecision::Rewards(rewards) => {
                let free_slot = run.state().potions.contains(&None);
                match next_claim(&mut passes, rewards, free_slot) {
                    Some(i) => RunAction::ClaimReward(i),
                    None if rewards.iter().any(|r| matches!(r, Reward::Card(_))) => break,
                    // A potion on a full belt cannot be taken; the player leaves the screen,
                    // which is a skip of the rest (`referee/tracer/Rewards.cs`).
                    None => RunAction::Proceed,
                }
            }
            RunDecision::Treasure(Some(_)) => RunAction::TakeRelic,
            RunDecision::Treasure(None) => RunAction::Proceed,
            // sts2-cli forces the map once a rest-site option is done, also when a relic
            // (`MiniatureTent`) keeps the other options open.
            RunDecision::RestSite(_) if run.legal_actions().contains(&RunAction::Proceed) => {
                RunAction::Proceed
            }
            // sts2-cli's `HeadlessCardSelector` takes a lone option that needs a pick itself.
            // The game shows the screen there when the selection needs manual confirmation
            // (Smith, a shop or reward card removal: `CardSelectCmd.FromDeckForUpgrade`,
            // `FromDeckGeneric`), so the engine asks and the session answers like the referee.
            RunDecision::CardSelect(select)
                if select.options.len() == 1 && select.min() >= 1 && select.picked.is_empty() =>
            {
                RunAction::SelectCard(0)
            }
            _ => break,
        };
        run.apply(next)?;
    }
    Ok(())
}

/// One open rewards screen's pass: the rewards it still shows and how many of them the pass has
/// gone over.
type Pass = (Vec<Reward>, usize);

/// The reward sts2-cli's pass claims next on the screen showing `rewards`, if any. sts2-cli's
/// reward loop and `TestMode`'s `RewardsSet.Offer` go over a set once, in reward order: a potion
/// that did not fit when its turn came stays, even if a later relic (`PotionBelt`) frees a slot.
/// `passes` holds one pass per open screen (a claimed relic may open one over it).
fn next_claim(passes: &mut Vec<Pass>, rewards: &[Reward], free_slot: bool) -> Option<usize> {
    match passes
        .iter()
        .rposition(|(shown, _)| same_screen(shown, rewards))
    {
        Some(i) => passes.truncate(i + 1),
        None => passes.push((rewards.to_vec(), 0)),
    }
    let pass = passes
        .last_mut()
        .expect("the screen was just found or pushed");
    let claimable = rewards.iter().skip(pass.1).position(|r| match r {
        Reward::Gold(_) | Reward::Relic(_) | Reward::WaxRelic(_) => true,
        Reward::Potion(_) => free_slot,
        // sts2-cli auto-collects gold, relics and potions only
        // (`RunSimulator.cs` reward loop), so a `SpecialCardReward` is never taken.
        Reward::Card(_) | Reward::SpecialCard(_) => false,
    });
    let i = pass.1 + claimable?;
    pass.0 = rewards.to_vec();
    pass.1 = i;
    pass.0.remove(i);
    Some(i)
}

/// Whether a screen that showed `shown` after the pass's last claim now shows `rewards`. A
/// claimed relic may change the cards of the card rewards still up (`OnRelicObtained`, see
/// `Run::modify_card_rewards_for_new_relic`), so those do not count.
fn same_screen(shown: &[Reward], rewards: &[Reward]) -> bool {
    shown.len() == rewards.len()
        && shown.iter().zip(rewards).all(|pair| match pair {
            (Reward::Card(_), Reward::Card(_)) => true,
            (a, b) => a == b,
        })
}

/// What the action ended: the referee still dumps a fight that ended on the action.
#[derive(Default)]
struct Advance {
    /// The fight the action ended, as it ended: the referee still dumps it. A fight can end on
    /// the action that enters its room (`FurCoat`'s 1-HP enemies, `FestivePopper`), and the
    /// reward claims sts2-cli makes after the action clear it from the run
    /// ([`Run::ended_combat`]).
    ended: Option<Box<Combat>>,
}

/// The referee action for an engine action; `Ok(None)` for a card pick that leaves its
/// selection unfinished.
fn referee_action(run: &Run, action: &RunAction) -> Result<Option<Value>> {
    Ok(Some(match action {
        RunAction::AncientOption(i) | RunAction::RestOption(i) | RunAction::EventOption(i) => {
            json!({"action": "choose_option", "args": {"option_index": i}})
        }
        RunAction::ChooseNode(c) => {
            json!({"action": "select_map_node", "args": {"col": c.col, "row": c.row}})
        }
        RunAction::Combat(a) => {
            let combat = run.combat().ok_or("no combat in progress")?;
            let mut referee = to_referee(combat, a)?;
            if let Action::UsePotion { slot, .. } = a {
                // The referee counts every potion in the belt, ported or not.
                let index = run.potions()[..*slot].iter().flatten().count();
                referee["args"]["potion_index"] = json!(index);
            }
            referee
        }
        RunAction::PickCard { card, .. } => {
            json!({"action": "select_card_reward", "args": {"card_index": card}})
        }
        RunAction::SkipReward(i) => match run.pending_decision() {
            RunDecision::Rewards(rewards) if matches!(rewards.get(*i), Some(Reward::Card(_))) => {
                json!({"action": "skip_card_reward"})
            }
            _ => return Err("only card rewards can be skipped on the referee".into()),
        },
        RunAction::SelectCard(i) => {
            let RunDecision::CardSelect(select) = run.pending_decision() else {
                return Err("no card selection".into());
            };
            if select.picked.len() + 1 < select.count || select.manual_confirm() {
                return Ok(None);
            }
            let indices: Vec<String> = select
                .picked
                .iter()
                .chain([i])
                .map(ToString::to_string)
                .collect();
            json!({"action": "select_cards", "args": {"indices": indices.join(",")}})
        }
        // The referee hears of the picks only when they are final.
        RunAction::DeselectCard(_) => return Ok(None),
        // A selection confirmed with fewer picks than its maximum (`GnarledHammer`).
        RunAction::ConfirmSelect => {
            let RunDecision::CardSelect(select) = run.pending_decision() else {
                return Err("no card selection".into());
            };
            let indices: Vec<String> = select.picked.iter().map(ToString::to_string).collect();
            json!({"action": "select_cards", "args": {"indices": indices.join(",")}})
        }
        RunAction::BuyCard(i) => json!({"action": "buy_card", "args": {"card_index": i}}),
        RunAction::BuyRelic(i) => json!({"action": "buy_relic", "args": {"relic_index": i}}),
        RunAction::BuyPotion(i) => json!({"action": "buy_potion", "args": {"potion_index": i}}),
        RunAction::RemoveCard => json!({"action": "remove_card"}),
        RunAction::Proceed if matches!(run.pending_decision(), RunDecision::Shop(_)) => {
            json!({"action": "leave_room"})
        }
        // sts2-cli's `use_potion` passes no target to an `AnyPlayer` potion, whose `OnUse`
        // then throws (`DoUsePotion`), so out-of-combat uses have no oracle.
        RunAction::UsePotion(_) => {
            return Err("out-of-combat potion use is not drivable on the referee".into());
        }
        // The policies never discard (`Run::legal_actions` does not list it).
        RunAction::DiscardPotion(_) => {
            return Err("potion discard is not drivable on the referee".into());
        }
        // sts2-cli's `skip_select` on a choose-a-card screen reports the next decision before
        // the relic's code after the screen ran (`HeftyTablet`'s `Injury` joins a line later,
        // and the finished Neow page is shown again), so a skip has no oracle.
        RunAction::CancelSelect
            if matches!(
                run.pending_decision(),
                RunDecision::CardSelect(CardSelect {
                    purpose: SelectPurpose::ChooseACard { .. },
                    ..
                })
            ) =>
        {
            return Err("referee skip_select".into());
        }
        // The game's own alternatives list (`CardRewardAlternative.Generate`, `Skip` first), which
        // `Run::card_reward_alternatives` follows (referee/tracer/CardAlternative.cs).
        RunAction::CardAlternative { alternative, .. } => {
            json!({"action": "card_alternative", "args": {"alternative": alternative}})
        }
        // In test mode the game takes the first bundle without a screen.
        RunAction::SelectBundle(_) => {
            return Err("a bundle choice is not drivable on the referee".into());
        }
        RunAction::ClaimReward(_)
        | RunAction::CancelSelect
        | RunAction::TakeRelic
        | RunAction::Proceed => {
            return Err(format!("{action:?} is decided by the referee, not the player").into());
        }
    }))
}

/// The referee's `decision` for the run's pending decision.
fn decision_name(run: &Run) -> &'static str {
    match run.pending_decision() {
        // The victory is `TheArchitect`'s event room; its dialogue options only lead to `PROCEED`.
        RunDecision::Ancient(_) | RunDecision::Event(_) | RunDecision::Victory => "event_choice",
        RunDecision::Map(_) => "map_select",
        // sts2-cli shows a card choice in combat as its own prompt.
        RunDecision::Combat(c) if c.pending_decision().is_some() => "card_select",
        RunDecision::Combat(_) => "combat_play",
        RunDecision::Rewards(_) => "card_reward",
        RunDecision::CardSelect(_) => "card_select",
        RunDecision::Bundles(_) => "bundle_select",
        RunDecision::RestSite(_) => "rest_site",
        RunDecision::Shop(_) => "shop",
        RunDecision::Treasure(_) | RunDecision::Unsupported(_) | RunDecision::Stopped(_) => {
            "unsupported"
        }
        RunDecision::GameOver => "game_over",
    }
}

/// A combat state in the referee's schema, minus the belt (the combat belt holds only the
/// ported potions, so the belt is compared on the run object instead) and the RNG counters
/// (also on the run object).
fn combat_state_json(combat: &Combat) -> Value {
    let mut state = state_json(combat.state());
    if let Some(state) = state.as_object_mut() {
        state.remove("rng");
    }
    if let Some(player) = state.get_mut("player").and_then(Value::as_object_mut) {
        player.remove("potions");
    }
    state
}

/// A deck card in `dump_run`'s schema. An unported card lists no `vars`, so they are not
/// compared.
fn card_json(card: &RunCard) -> Value {
    let mut json = json!({
        "id": card.id,
        "upgrade": card.upgrade_level,
        "enchantment": card.enchantment.map(|e| e.kind.id()),
        "enchantment_amount": card.enchantment.map(|e| e.amount),
    });
    if let Some(f) = ids::card_from_id(card.id)
        .and_then(|kind| fields::card(kind, &cards::data_from_deck(kind, card.counter)))
    {
        json["fields"] = f;
    }
    if let Some(vars) = card.vars() {
        let vars: Vec<Value> = vars
            .iter()
            .map(|v| {
                let mut entry = vec![json!(v.name.to_string()), decimal_json(v.value)];
                entry.extend(v.calculated.map(decimal_json));
                Value::Array(entry)
            })
            .collect();
        json["vars"] = Value::Array(vars);
    }
    json
}

/// An `f32` as the referee prints it: the shortest string that round-trips, read back as JSON. A
/// value JSON has no number for (NaN, infinite) stays a string, which no referee number matches.
fn f32_json(value: f32) -> Value {
    let text = value.to_string();
    serde_json::from_str(&text).unwrap_or(Value::String(text))
}

/// The run object in `dump_run`'s schema, restricted to fields the engine models.
/// `ended` is the fight that just ended, if any: the game keeps the card a relic remembered when
/// the fight ends without `AfterCombatEnd` (the player died; `Vambrace.TriggeringCard`).
fn run_json(run: &Run, ended: Option<&CombatState>) -> Value {
    let s = run.state();
    // During a fight the live HP, gold and streams are the combat's.
    let (hp, max_hp, gold, rng) = run.combat().map_or((s.hp, s.max_hp, s.gold, &s.rng), |c| {
        let cs = c.state();
        let player = cs.creature(cs.player.creature);
        (player.hp, player.max_hp, cs.player.gold, &cs.rng)
    });
    // A fight changes deck versions as it goes (`TheScythe`, `GeneticAlgorithm`, `Goopy`) and
    // removes deck cards at once (`SwipePower.Steal`: `CardPileCmd.RemoveFromDeck`); the run
    // reads them back only after it, so show the combat's.
    let mut deck = s.deck.clone();
    if let Some(c) = run.combat() {
        let cs = c.state();
        for (card, version) in deck.iter_mut().zip(&cs.deck) {
            card.counter = version.counter;
            card.enchantment = version.enchantment;
        }
        let mut index = 0..;
        deck.retain(|_| {
            index
                .next()
                .is_some_and(|i| !cs.removed_from_deck.contains(&i))
        });
    }
    let counters: serde_json::Map<String, Value> = RunRngType::ALL
        .iter()
        .map(|&t| (t.enum_name().to_owned(), json!(rng[t].counter())))
        .collect();
    let player_counters: serde_json::Map<String, Value> = PlayerRngType::ALL
        .iter()
        .map(|&t| (t.enum_name().to_owned(), json!(s.player_rng[t].counter())))
        .collect();
    // In a fight the relics' fields are the combat's.
    let relic_state: Vec<Value> = run.combat().map_or_else(
        || {
            s.relics
                .iter()
                .map(|r| fields::relic_json(ended, r.kind, &r.data))
                .collect()
        },
        |c| {
            let cs = c.state();
            cs.player
                .relics
                .iter()
                .map(|r| fields::relic_json(Some(cs), r.kind, &r.data))
                .collect()
        },
    );
    let act = s.act();
    let room_type = s.room_type().map(sts2_core::run::RoomType::name);
    let room_class = s.room_type().map(sts2_core::run::RoomType::room_class);
    let unknown = |r| f32_json(s.unknown_odds.get(r).unwrap_or_default());
    json!({
        "act": s.act_index + 1,
        "act_id": act.act.id,
        "floor": s.act_floor,
        "total_floor": s.total_floor(),
        "coord": s.coord().map(|c| json!([c.col, c.row])),
        "visited": s.visited.iter().map(|c| json!([c.col, c.row])).collect::<Vec<_>>(),
        "room": room_class,
        "room_type": room_type,
        "hp": hp,
        "max_hp": max_hp,
        "gold": gold,
        "deck": deck.iter().map(card_json).collect::<Vec<_>>(),
        "relics": s.relics.iter().map(|r| r.id()).collect::<Vec<_>>(),
        "relic_state": relic_state,
        "removals_used": s.card_shop_removals_used,
        "potions": run.potions(),
        "rng": {"run": counters, "player": player_counters},
        "odds": {
            "card_rarity_offset": f32_json(s.card_rarity_odds.offset),
            "potion_reward": f32_json(s.potion_reward_odds.value),
            "unknown_monster": unknown(sts2_core::run::RoomType::Monster),
            "unknown_elite": unknown(sts2_core::run::RoomType::Elite),
            "unknown_treasure": unknown(sts2_core::run::RoomType::Treasure),
            "unknown_shop": unknown(sts2_core::run::RoomType::Shop),
        },
        "rooms": {
            "events_visited": act.events_visited,
            "normal_visited": act.normal_encounters_visited,
            "elite_visited": act.elite_encounters_visited,
            "boss_visited": act.boss_encounters_visited,
        },
        "events_seen": events_seen(s),
        "pending": pending_json(run, s),
    })
}

/// The run object of an act's first line (step 0 and the line that enters the next act): with
/// the act's map and its pre-rolled room lists.
fn act_start_run_json(run: &Run, ended: Option<&CombatState>) -> Value {
    let mut out = run_json(run, ended);
    let act = run.state().act();
    out["rooms"]["events"] = json!(act.events);
    out["rooms"]["normals"] = json!(
        act.normal_encounters
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );
    out["rooms"]["elites"] = json!(
        act.elite_encounters
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );
    out["rooms"]["boss"] = json!(act.boss.id);
    out["rooms"]["second_boss"] = json!(act.second_boss.map(|e| e.id));
    out["rooms"]["ancient"] = json!(act.ancient);
    out["map"] = map::to_json(&run.state().map);
    out
}

/// An event page in `dump_run`'s `pending` schema.
fn event_json(room: &EventRoom) -> Value {
    json!({
        "kind": "event",
        "event": room.kind.id(),
        "finished": room.finished.is_some(),
        "options": room
            .options
            .iter()
            .map(|o| json!({
                "text_key": o.text_key,
                "locked": o.action.is_none(),
                "chosen": false,
            }))
            .collect::<Vec<_>>(),
    })
}

/// `VisitedEventIds`, sorted like the referee prints them.
fn events_seen(s: &RunState) -> Vec<&'static str> {
    let mut seen = s.visited_event_ids.clone();
    seen.sort_unstable();
    seen
}

/// `run.pending` in `dump_run`'s schema, on the modeled keys.
fn pending_json(run: &Run, s: &RunState) -> Value {
    match run.pending_decision() {
        RunDecision::Ancient(room) => json!({
            "kind": "event",
            "event": room.kind.id(),
            "finished": false,
            "options": room
                .options
                .iter()
                .map(|o| json!({"text_key": o.text_key(room.kind)}))
                .collect::<Vec<_>>(),
        }),
        RunDecision::Event(room) => event_json(room),
        // Death in an event leaves its page up.
        RunDecision::GameOver if let Some(room) = run.event() => event_json(room),
        // A card reward outside a fight (an event's, Neow's, a shop relic's) is sts2-cli's
        // blocking card choice, shown without `can_skip`.
        RunDecision::Rewards(rewards) if !matches!(s.room, Room::Combat { .. }) => {
            let cards = rewards.iter().find_map(|r| match r {
                Reward::Card(cards) => Some(cards.cards.iter().map(card_json).collect::<Vec<_>>()),
                _ => None,
            });
            json!({"kind": "event_card_reward", "cards": cards})
        }
        // The tracer lists the current point's children, also when free travel allows more, and
        // the start point and its children while no point of the act is visited (a new act's
        // map screen, or a forced `act_index`).
        RunDecision::Map(_) => s
            .coord()
            .and_then(|c| s.map.point(c))
            .map_or_else(|| act_start_map_json(s), |p| map_json(p.children.to_vec())),
        RunDecision::Rewards(rewards) => {
            let cards = rewards.iter().find_map(|r| match r {
                Reward::Card(cards) => Some(cards.cards.iter().map(card_json).collect::<Vec<_>>()),
                _ => None,
            });
            json!({"kind": "card_reward", "cards": cards, "can_skip": true})
        }
        RunDecision::CardSelect(select @ CardSelect { count, .. }) => {
            // `FromChooseACardScreen` asks the selector for 0 to 1 cards.
            json!({
                "kind": "card_select",
                "cards": select.cards(&s.deck).iter().map(card_json).collect::<Vec<_>>(),
                "min": select.min(),
                "max": count,
            })
        }
        RunDecision::Shop(shop) => shop_json(s, shop),
        RunDecision::RestSite(options) => json!({
            "kind": "rest",
            "options": options
                .iter()
                .map(|o| json!({"id": o.kind.id(), "enabled": o.enabled}))
                .collect::<Vec<_>>(),
        }),
        RunDecision::Combat(c) => match c.pending_decision() {
            Some(DecisionRequest::ChooseCards { min, options, .. }) => {
                let cs = c.state();
                let options = referee_order(cs, options);
                json!({
                    "kind": "card_select",
                    "cards": options
                        .iter()
                        .map(|&id| {
                            let card = cs.card(id);
                            let mut json = json!({
                                "id": card.kind.entry(),
                                "upgrade": card.upgrade_level,
                                "enchantment": card.enchantment.map(|e| e.kind.id()),
                                "enchantment_amount": card.enchantment.map(|e| e.amount),
                            });
                            if let Some(f) = fields::card(card.kind, &card.data) {
                                json["fields"] = f;
                            }
                            json
                        })
                        .collect::<Vec<_>>(),
                    // The engine caps `max` at the options (a "no limit" prefs `MaxSelect` is
                    // 999999999 in the game), so only `min` is compared.
                    "min": min,
                })
            }
            None => Value::Null,
        },
        RunDecision::Victory => json!({"kind": "event", "event": THE_ARCHITECT}),
        RunDecision::Treasure(_)
        | RunDecision::Unsupported(_)
        | RunDecision::Stopped(_)
        | RunDecision::GameOver
        | RunDecision::Bundles(_) => Value::Null,
    }
}

/// A map decision's `next` points, sorted like the tracer lists them.
fn map_json(mut next: Vec<MapCoord>) -> Value {
    next.sort_by_key(|c| (c.row, c.col));
    json!({
        "kind": "map",
        "next": next.iter().map(|c| json!([c.col, c.row])).collect::<Vec<_>>(),
    })
}

/// The map decision of an act no point of which is visited: its start and the start's children.
fn act_start_map_json(s: &RunState) -> Value {
    let start = s.map.start();
    map_json(
        std::iter::once(start.coord)
            .chain(start.children.iter())
            .collect(),
    )
}

/// A shop in `dump_run`'s `pending` schema. The removal's `used` is left out: sts2-cli has no
/// merchant UI node, which is what marks the entry used (`NMerchantCardRemoval.OnCardRemovalUsed`).
fn shop_json(s: &RunState, shop: &Shop) -> Value {
    let price = |base| shop::price(s, base);
    json!({
        "kind": "shop",
        "cards": shop
            .cards
            .iter()
            .map(|c| json!({
                "card": c.card.as_ref().map(card_json),
                "cost": price(c.base_cost),
                "on_sale": c.on_sale,
                "stocked": c.card.is_some(),
            }))
            .collect::<Vec<_>>(),
        "relics": shop
            .relics
            .iter()
            .map(|r| json!({"id": r.id, "cost": price(r.base_cost), "stocked": r.id.is_some()}))
            .collect::<Vec<_>>(),
        "potions": shop
            .potions
            .iter()
            .map(|p| json!({"id": p.id, "cost": price(p.base_cost), "stocked": p.id.is_some()}))
            .collect::<Vec<_>>(),
        "removal": {"cost": price(shop.removal_base_cost)},
    })
}

/// The act a run-mode actions file (or a run loadout) forces: its `"act1"`, `OVERGROWTH` (the
/// referee's default) or `UNDERDOCKS`, any case (`referee.py` `run_config`).
///
/// # Errors
/// If `"act1"` is not a string naming one of the two.
pub fn act1_of(file: &Value) -> Result<Act1> {
    file.get("act1").map_or(Ok(Act1::Overgrowth), |v| {
        v.as_str()
            .map(str::to_lowercase)
            .and_then(|key| Act1::from_key(&key))
            .filter(|&a| a != Act1::Random)
            .ok_or_else(|| format!("unknown act1 {v} (OVERGROWTH or UNDERDOCKS)").into())
    })
}

/// The `"act1"` field `referee.py` reads for `act1`: none for Overgrowth, its default, so actions
/// files (and the trace store's keys) stay as they were.
#[must_use]
pub fn act1_field(act1: Act1) -> Option<Value> {
    (act1 == Act1::Underdocks).then(|| json!("UNDERDOCKS"))
}

/// The engine's trace of a run-mode referee actions list at `ascension`, step 0 first.
///
/// Each referee action is matched to the engine action that sends it (a run card selection's
/// `select_cards` is one pick per index; a combat card choice is the whole choice). Stops early
/// once the run is over.
///
/// # Errors
/// If an action has no engine equivalent or the run rejects it.
pub fn replay(
    character: Character,
    seed: &str,
    ascension: u8,
    mode: (bool, Act1),
    actions: &[Value],
) -> Result<Vec<Value>> {
    replay_session(character, seed, ascension, mode, actions).map(|(_, trace)| trace)
}

/// [`replay`], also returning the session where it ended.
///
/// # Errors
/// As [`replay`].
pub fn replay_session(
    character: Character,
    seed: &str,
    ascension: u8,
    (real_game, act1): (bool, Act1),
    actions: &[Value],
) -> Result<(RunSession, Vec<Value>)> {
    let mut session = RunSession::new(character, seed, ascension, real_game, act1)?;
    let mut trace = vec![session.start_line()];
    for action in actions {
        if matches!(
            session.run.pending_decision(),
            RunDecision::GameOver | RunDecision::Victory
        ) {
            break;
        }
        let line = if action["action"] == "run_loadout" {
            Some(session.apply_loadout(&action["args"])?)
        } else if action["action"] == "enter_event" {
            let id = action["args"]["event"]
                .as_str()
                .ok_or("enter_event needs an event")?;
            Some(session.enter_event(id)?)
        } else if let Some(combat) = session.run.combat()
            && action["action"] == "select_cards"
        {
            let action = RunAction::Combat(crate::session::from_referee(combat, action)?);
            session.apply(&action)?
        } else if let RunDecision::CardSelect(_) = session.run.pending_decision() {
            let indices = action["args"]["indices"]
                .as_str()
                .ok_or("select_cards needs indices")?;
            let mut line = None;
            for index in indices.split(',').filter(|i| !i.is_empty()) {
                line = session.apply(&RunAction::SelectCard(index.parse()?))?;
            }
            // A selection that takes fewer picks than its maximum is confirmed.
            if matches!(session.run.pending_decision(), RunDecision::CardSelect(_)) {
                line = session.apply(&RunAction::ConfirmSelect)?;
            }
            line
        } else {
            let engine_action = session
                .run
                .legal_actions()
                .into_iter()
                .find(|a| referee_action(&session.run, a).ok().flatten().as_ref() == Some(action))
                .ok_or_else(|| format!("no engine action sends {action}"))?;
            session.apply(&engine_action)?
        };
        let (_, line) = line.ok_or_else(|| format!("{action} left its selection unfinished"))?;
        trace.push(line);
    }
    Ok((session, trace))
}

#[cfg(test)]
mod tests {
    use super::{Pass, next_claim};
    use sts2_core::run::rewards::CardReward;
    use sts2_core::run::{Reward, RunCard};

    fn card_reward(upgraded: bool) -> Reward {
        let mut card = RunCard::new("STRIKE_NECROBINDER");
        if upgraded {
            card.upgrade();
        }
        Reward::Card(CardReward {
            cards: vec![card],
            options: None,
            reroll_options: None,
            count: 1,
            event_rng: false,
            can_reroll: false,
            upgrade_generated: false,
        })
    }

    #[test]
    fn a_skipped_potion_stays_when_a_relic_changes_the_card_reward() {
        // A Black Star elite on a full belt: the potion is passed over, Potion Belt frees a
        // slot, then an egg upgrades the card reward still up. The pass does not go back.
        let mut passes: Vec<Pass> = Vec::new();
        let potion = Reward::Potion("FIRE_POTION");
        let screen = [
            potion.clone(),
            Reward::Relic("POTION_BELT"),
            Reward::Relic("FROZEN_EGG"),
            card_reward(false),
        ];
        assert_eq!(next_claim(&mut passes, &screen, false), Some(1));
        let after_belt = [
            potion.clone(),
            Reward::Relic("FROZEN_EGG"),
            card_reward(false),
        ];
        assert_eq!(next_claim(&mut passes, &after_belt, true), Some(1));
        let after_egg = [potion.clone(), card_reward(true)];
        assert_eq!(next_claim(&mut passes, &after_egg, true), None);
        // A screen opened over it is a new pass, from its first reward.
        assert_eq!(next_claim(&mut passes, &[potion], true), Some(0));
        assert_eq!(passes.len(), 2);
    }
}
