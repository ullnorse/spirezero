# Referee: the real game, headless

Parity oracle for the Rust engine. Runs the shipped v0.111.0 `sts2.dll` with no UI, plays a scripted
action list and prints the full combat state after every action as JSONL. Differential tests only.

Built on [morefreeze/sts2-cli](https://github.com/morefreeze/sts2-cli) (MIT), which provides the Godot
stubs, the JSON stdin/stdout protocol and the IL patches that make the game run without a window.
It is cloned (pinned to commit `7662fde`) into `referee/vendor/`, which is gitignored, as are the copied
game DLLs in its `lib/`. Nothing from the game or from sts2-cli is committed here.

## Use

    referee/setup.sh                                  # clone, copy DLLs, patch, build into referee/builds/<hash> (~1 min); a rerun with unchanged sources only re-points `current`
    referee/run.sh <seed> <actions.json> > trace.jsonl
    referee/referee.py serve                          # many seeds, one process: `<seed>\t<actions.json>\t<trace.jsonl>` lines in, `ok`/`fail <reason>` out
    referee/referee.py client                         # the same, through the tools/slot.sh slot's daemon when $SPIREZERO_SLOT_SOCKET is set
    referee/find_seed.sh [ENCOUNTER_ID] [--start N] [--limit N]

Builds are immutable: `setup.sh` builds into `referee/builds/<hash of setup.sh + tracer/*.cs>/` and then switches the `current` symlink
atomically; it never touches an existing build, so runs in progress (they resolve the build once at start) and other worktrees are safe
while one worktree runs `setup.sh`. A failed build leaves nothing behind. `run.sh` uses `$STS2_REFEREE` (a build directory or `current` link) if set,
else the main checkout's `referee/builds/current`, else this checkout's. A worktree that changes `setup.sh` or `tracer/` must build and
use its own (`STS2_REFEREE=$PWD/referee/builds/current`). `referee.py where` prints the build in use. Old builds are not pruned (`rm -rf` the ones no run uses).

Needs `~/.dotnet` (.NET 9 SDK, see `tools/golden/README.md`), `python3`, and the game install.
`STS2_GAME` overrides the game directory (default is the one in `tools/golden/golden.csproj`).

Actions file, see `examples/`: `{"character", "ascension", "encounter", "neow", "actions": [...]}`.
Each action is `{"action": <sts2-cli action>, "args": {...}}`; the useful ones are
`play_card {card_index, target_index}`, `end_turn`, `use_potion`, `select_cards {indices: "0,1"}`.
`card_index` indexes the current hand. With `encounter` the fight is forced via sts2-cli's `enter_room`;
without it the harness plays through Neow (option `neow`) and the first map node.

### Parity runs

`tools/diff.sh <seeds> [encounter]` (env `ASC` for the ascension, `LOADOUT` for a loadout file) plays the engine against the referee seed by seed.
`tools/regress.sh [-j N] [-v] [seeds]` runs every encounter and every loadout (on the encounters its `"encounters"` key names, default
`FuzzyWurmCrawlerWeak` and `SlimesWeak`; `"ascensions": [..]` limits the ascensions) at ascension 0 and 10, `-j` runs in parallel (default half the cores),
prints failures and one summary line, and skips a run whose (source hash, referee build, loadout, encounter, ascension, seeds) already passed
(`~/.cache/spirezero-regress-cache`); a run that is not skipped still replays the engine but takes its referee traces from the trace store below, so an
engine-only change plays no game whose actions did not change. Logs of the runs are in `target/regress/`. The engine's ascension starting deck (Ascender's Bane from 5) and HP
match the referee's (`a10_*` fixtures).

The combat comparison covers the RNG counters (`rng.run`; the `UpFront` stream, which `start_run`'s map generation draws, is compared as draws since step 0),
the card fields of every pile, and a torn-down terminal state (after a win: no powers or block on the player, empty piles, no Osty, no enemies).
Only the potion belt after a win is lenient, for traces of builds before T17, in which sts2-cli claimed the fight's potion reward by itself.
A forced fight no longer claims any reward (see "Where sts2-cli is not the real game").

### Trace store (T12)

The referee runs only for inputs it has never seen. `referee.py run` and every `serve`/`client` job look the input up in
`~/.cache/spirezero-referee-traces/<key>.jsonl.gz` (gzip; plain `<key>.jsonl` entries of older stores are still read, `python3 referee/trace_cache.py compress` converts them, DK1) first (`referee/trace_cache.py`; `REFEREE_TRACE_CACHE` elsewhere) and on a hit
copy the stored trace without starting a game or contacting the slot daemon. The key is the referee build (name plus
`sts2.dll`/`Sts2Headless.dll` size and mtime), `referee.py`'s hash, every `STS2_*` variable the game reads (not `STS2_ACT1`/
`STS2_REAL_*`, which come from the file), `DOTNET_ROOT`, the seed and the actions file as canonical JSON (character, ascension,
encounter, loadout, mode, act1, `real_*` and the actions; `MAX_ACTIONS` and the policy only shape the actions). A miss plays the
game and writes the trace with a temp file and rename, so concurrent workers are safe. A failed run is stored with
`<key>.fail` (the reason) only from `run` (a fresh process); `serve`'s failures are replayed every time (a failure there
is already a fresh process's, see "One process per slot", but stays conservative). `REFEREE_NO_TRACE_CACHE=1` bypasses the store,
`REFEREE_STATS=<file>` logs `hit`/`miss`/`game` lines (regress prints the totals). Nothing is pruned; deleting any part of the
store is safe (`find ~/.cache/spirezero-referee-traces -atime +30 -delete`). A full `regress.sh 3` stores ~8,500 traces, ~12 MB compressed. `tools/disk.sh --gc` drops traces unread for 30 days.

### Custom loadout

Forced-encounter files may add `"loadout": {"deck": [...], "relics": [...], "potions": [...], "hp": N, "max_hp": N}`; every key is optional,
ids are SCREAMING_SNAKE and `+1` upgrades a card once. A deck entry may end in `:ENCHANTMENT` or `:ENCHANTMENT*AMOUNT`
(after any `+N`; amount 1 by default), applied with `CardCmd.Enchant`: `"STRIKE_NECROBINDER+1:VIGOROUS*3"`. `deck` and `relics` replace the starters in the given order, relics go through
`RelicCmd.Obtain` (so `AfterObtained` runs and can consume RNG, see `docs/semantics.md`), `potions` fill slots in order, and `hp`/`max_hp`
are applied last. An unknown id or too many potions aborts with the offending id. Implemented by the `set_loadout` command (`tracer/Loadout.cs`),
sent before `enter_room`, so the initial shuffle uses the given deck order. See `examples/loadout.json`.
If the fight opens a prompt before the first play (`GamblingChip`), step 0 shows it (`card_select`) and the first actions answer it.
The combat dump lists a card's `enchantment` (`{id, amount}`) only when it has one.

### Run loadout (H2)

Run mode takes a `run_loadout` action (not an sts2-cli one; `referee.py` sends the command of that name, `tracer/RunLoadout.cs`) once the run waits on the
map after Neow: `{"action": "run_loadout", "args": {"add_cards": [...], "gold": N, "add_relics": [...], "max_hp": N, "hp": N}}`, every key optional and
applied in that order. Cards use the `set_loadout` syntax (`ID+1:ENCHANTMENT*A`) and join the deck with `AddInternal` (no deck hooks), `gold` sets the gold,
each relic goes through `RelicCmd.Obtain` on a task, so `AfterObtained` runs; a prompt it opens (`DollysMirror`, `Kifuda`, `Orrery`) is the answer's
`decision` and the next actions answer it (only the last relic may prompt, and `hp`/`max_hp` cannot follow it). A relic whose reward set TestMode cannot
take whole (`Cauldron`, 5 potions on 3 slots) leaves the relic obtained and the potions it fit. `sts2-diff run-gen --run-loadout FILE` and
`RUN_LOADOUT=FILE tools/diff_run.sh N` apply the same file on the engine after Neow; the files are `crates/sts2-diff/run_loadouts/*.json`, which
`tools/regress_runs.sh` replays (a file may add `"policy"` and `"ascensions"`).

### Forced events (XT2)

Run mode takes an `enter_event` action (ours, like `run_loadout`): `{"action": "enter_event", "args": {"event": "ABYSSAL_BATHS"}}` sends sts2-cli's
`enter_room` command with type `event`, which is `RunManager.EnterRoom(new EventRoom(...))`: the current room is exited and the event begins, with no
history entry or floor, nothing pulled from the act's event list and no `IsAllowed` check (`docs/semantics/run.md`). Sent while the run waits on the
map (after Neow and any `run_loadout`), the event's pages, prompts and fights trace like a natural one and a finished event returns to the same map.
The engine's side is `Run::debug_enter_event`. `EVENT=<ID> tools/diff_run.sh N` (or a run loadout's `"event"`) forces the event on both sides, takes
first-page option `<seed index> % <options>` and stops (`event_done`) back on the map; `tools/diff_events.sh <ID>...` runs it for the random and greedy
policies and lists the options taken. Every ported event can be diffed this way without a natural run.

### Character and act

Both modes take `"character"` (`Ironclad`, `Silent`, `Defect`, `Regent`, `Necrobinder`; default Necrobinder) and a loadout is built for that character, so
`"deck"` ids must be that character's cards. Examples: `examples/character_<name>.json` (a forced fight per character; Defect plays Zap and Dualcast, Regent
Falling Star and Venerate). Run mode also takes `"act1": "UNDERDOCKS"` (default `"OVERGROWTH"`, which is the game default for the referee): `setup.sh` patches
`StartRun` to pass `acts: [Underdocks, Hive, Glory]` to `RunState.CreateForTest` when the process has `STS2_ACT1=underdocks`, which `referee.py` sets from the
file. The game's own `GetRandomList` act roll is not reached (see `docs/specs/underdocks.md`), so it can't be diffed. `examples/run_underdocks.json` (seed `RL1S0`)
plays Neow, a row-1 fight (`CORPSE_SLUG` x2) and two turns. A forced fight (`"encounter"`) is independent of the act. `act1` is rejected outside run mode.

### Real-game branches (run mode)

The referee runs with `TestMode.IsOn`, which skips three gameplay branches of the real game (`docs/specs/test-mode.md`). `referee/patcher` (a Mono.Cecil
step in `setup.sh`) makes each opt-in: `"real_shop_prices": true` (shop potion prices roll `Shops.NextFloat(0.95, 1.05)`), `"real_cauldron": true`
(5 rolled `PotionReward`s), `"real_calling_bell": true` (Common/Uncommon/Rare `RelicReward`s) in the actions file. Unset, the build is unchanged.
`REAL_SHOP=1 POLICY=shopper tools/diff_run.sh 50` diffs the engine's real-game path (`Run` without `in_test_mode`) this way.

### Run mode

`{"mode": "run", "character", "ascension", "actions": [...]}` plays a whole run from the seed instead of one fight (no `encounter`,
`neow` or `loadout` keys; the first action answers the Neow event). Actions are sts2-cli's own, applied in order, with the same
`{"action", "args"}` shape. The decision after each one decides which is legal next:

| Decision (`decision` in the trace line) | Actions |
|---|---|
| `event_choice` (Neow, `?` events) | `choose_option {option_index}` |
| `map_select` | `select_map_node {col, row}` (a row-1 node after Neow, then a child of the current node) |
| `combat_play` | `play_card {card_index, target_index}`, `end_turn`, `use_potion {potion_index, target_index}`, `discard_potion {potion_index}` |
| `card_reward` | `select_card_reward {card_index}`, `skip_card_reward`, `card_alternative {alternative}` (ours, PX21: index into the game's alternatives, Skip first; Driftwood's REROLL, PaelsWing's SACRIFICE) |
| `card_select` (Smith, removal, transform, event picks) | `select_cards {indices: "0,1"}`, `skip_select` |
| `shop` | `buy_card {card_index}`, `buy_relic {relic_index}`, `buy_potion {potion_index}`, `remove_card`, `leave_room` |
| `rest_site` | `choose_option {option_index}` (HEAL, SMITH, ...; Smith then raises `card_select`, a heal with `DreamCatcher` `card_reward`) |
| `bundle_select` | `select_bundle {bundle_index}` |
| any non-combat room | `proceed` / `leave_room` (also what ends a boss reward screen) |
| `game_over` | none: the run is over; the line has `"victory": true` if it was won (TheArchitect's `PROCEED`) |

Indices are positions in the lists of `run.pending` (see below). `examples/run_act1.json` (seed `T6S0`) plays Neow, five fights with
four card picks, a `?` event, a `?` that rolls a treasure room and a rest site, and ends on the map at floor 8.

Each trace line is `{step, action, decision, state, run}`. `state` is the combat dump described below (it only carries
`seed/act/floor` outside combat and is unchanged from combat-only mode). `run` is the new read-only `dump_run` command
(`tracer/RunDump.cs`, added by `setup.sh`):

- `act`, `act_id`, `floor` (act-local, Neow is floor 1), `total_floor`, `coord` (current map coord or `null`), `visited` coords.
- `room`/`room_type`, `hp`, `max_hp`, `gold`, `deck` (`{id, upgrade, enchantment}` in order), `relics` (in order), `potions` (slot list, `null` = empty).
- `rng.run` and `rng.player`: call count of every `RunRngSet` / `PlayerRngSet` stream (`Rng._counter`, read through `ToSerializable()`).
  The map stream (`act_1_map`) is not a `RunRngSet` member and is not traced; the map itself is.
- `odds`: f32 values `card_rarity_offset`, `potion_reward`, `unknown_monster/elite/treasure/shop`.
  JSON prints the shortest string that round-trips the f32 (`0.06999999` is the exact f32 value).
- `rooms`: `events_visited`, `normal_visited`, `elite_visited`, `boss_visited` every line; with the map also the pre-rolled lists
  `events`, `normals`, `elites`, `boss`, `second_boss`, `ancient` of the current act. `events_seen` is `VisitedEventIds`, sorted.
- `pending`: the choice the run is waiting on, or `null` in combat. `kind` is one of `event` (`event`, `finished`, `options [{text_key, locked, chosen}]`),
  `card_reward` (`cards`, `can_skip`), `event_card_reward`, `card_select` (`cards`, `min`, `max`), `rest` (`options [{id, enabled}]`),
  `shop` (`cards [{card, cost, on_sale, stocked}]` character cards then colourless, `relics`, `potions` (`{id, cost, stocked}`), `removal {cost, used}`)
  or `map` (`next`: the coords the player can travel to).
- Only on step 0 and the first line of each new act: `map` (`nodes` with `col,row,type,children`, sorted by row then col, plus `start`, `boss`,
  `second_boss`) and, on step 0, `unlock_number_of_runs`. Children are sorted, so the dump does not expose `HashSet` order.
- With `"act_maps": true` in the actions file, step 0 also has `act_maps`: every act's standard map (same shape), built like
  `StandardActMap.CreateFor` for that act (map rng `act_<n>_map`, second boss from `HasSecondBoss`, no map hooks). `tools/diff_map.sh` uses it with `ACTS`.

## Where sts2-cli is not the real game

The trace reflects the game's own objects, but sts2-cli drives them, and in a few places it decides for the player. A diff test must account for these:

- sts2-cli's `start_run` handed the seed to `RunState.CreateForTest` raw, but the real game canonicalises a typed seed first
  (`SeedHelper.CanonicalizeSeed` in `StartRunLobby.BeginRunForAllPlayersIfAllReady`). `setup.sh` patches `StartRun` to call it,
  so seeds with `O`, `I`, lower case or surrounding whitespace match the engine (PX4).
- `HealBetweenActs` (heal to full on `proceed` after a boss, x0.8 at ascension 2 and above) is sts2-cli's invention. `setup.sh` comments out both call sites
  (`sed` on `RunSimulator.cs`), so nothing heals between acts except what the game itself does (the next act's Ancient event). Not exercised by the examples
  (they stop in Act 1), so whether the headless build runs that Ancient heal is unverified.
- Combat rewards: gold, relics and potions are claimed automatically in reward order before the card choice is shown, so a potion or relic reward can't be skipped
  and the claim order can't be chosen. Only the first card reward of a screen is offered. Card, gold, relic and potion generation is the game's own `RewardsSet`.
  In a forced fight (`encounter`) they are generated but not claimed (T17): the trace is diffed as a fight, and a claimed relic's `AfterObtained` changed the final state (Whetstone's `StableShuffle` drew `Niche`,
  Strawberry would raise max HP), which the engine's fight never does.
- Treasure rooms auto-pick relic 0 (and take any gold) when entered; the pick/skip choice isn't drivable. A `?` that rolls a treasure shows up as a
  `map_select` line with a new relic.
- Events that start a fight (`EnterCombatWithoutExitingEvent`, e.g. Dense Vegetation's REST) don't enter the combat: sts2-cli leaves the event on the same page and a second
  `choose_option` leaves for the map. Don't script those options; the trace would have no fight where the real game has one.
- An event's card reward (Brain Leech RIP, The Future of Potions) shows as `card_reward` with `pending.kind` `event_card_reward`. `setup.sh` patches
  `skip_card_reward` there to skip the rewards set like the game's Proceed button (otherwise the event never finishes) and waits for the event after a pick or skip.
- Jungle Maze Adventure's JOIN_FORCES throws headless (`NDebugAudioManager.Instance` is null) before giving the gold; don't script it.
- A rest-site heal's rewards (`DreamCatcher`'s card reward, `RewardsCmd.OfferCustom`) block the option on the game's card choice. sts2-cli used to force
  the map past it and drop the reward; `setup.sh` now stops there and shows it like an event's (`card_reward`, `pending.kind` `event_card_reward`,
  answered with `select_card_reward`/`skip_card_reward`), then leaves for the map. `crates/sts2-diff/run_loadouts/t17-dream_catcher.json` covers it.
- Combat commands wait for the game (T18, `tracer/RunWaits.cs`). sts2-cli's `end_turn` stopped waiting once `TurnStarted` fired, which the game also fires
  when the enemy turn starts, and its decision check reported `combat_play` after a fixed 100 ms; when the enemy turn yielded (PunchOff's event fight,
  49 of 58 seeds non-repeatable) the command returned mid-turn and the next one raced the game thread (stale hand, `NullReferenceException`, "Collection was modified").
  Now a combat command returns once a prompt is open, the combat has ended (a death only marks the loss until the game's next safe point), or the player's
  play phase has begun with the action queue empty; `end_turn` also needs the player's `TurnNumber` to have advanced, and its cancel-and-retry fallbacks are gone.
  A wait (including the action-executor wait) that takes over 30 s is an error on that step, not a stale state.
- sts2-cli reported victory once any boss of the last act was beaten, including the first of the two A10 (`DoubleBoss`) bosses. `setup.sh` replaces
  both of its shortcuts with `tracer/RunEnd.cs` `AfterBoss`, which follows the game: the first A10 boss returns to the map (its only next point is the
  second boss), and the last boss does `ActFloor++` and `EnterNextAct`, which enters TheArchitect's event room (`event_choice`, one dialogue option and
  then `PROCEED`); its `WinRun` ends the run and kills the player, and the next line is `game_over` with `"victory": true` (a death has no `victory`
  key). Bosses before the last act are unchanged (sts2-cli's `EnterNextAct`). Checked with a scratch act-jump build, Ironclad A0 and A10.
- sts2-cli rolls a won fight's room rewards itself (`DetectPostCombatState`, `WithRewardsFromRoom`) and ignored
  `EncounterModel.ShouldGiveRewards`, so the BattlewornDummy fights showed a gold/potion/card reward the game never offers
  (`NCombatUi.OnCombatWon` goes to `ProceedWithoutRewards`). `setup.sh` skips the rewards for such an encounter. (C24)
- Leaving such a fight resumes the event (`ProceedFromTerminalRewardsScreen`, `ResumePreviousRoom`), whose `BattlewornDummy.Resume` offers the
  room's `ExtraRewards` (TheHunt's card reward) through the blocking `GetSelectedCardReward`. sts2-cli ran it on the command's thread, which blocked
  300 s and skipped the reward; the resume now runs on a task (`tracer/RunWaits.cs` `ResumeEventOnTask`) and the reward is shown as an event
  `card_reward` in the event room. (RF1)
- sts2-cli's test selector (`HeadlessCardSelector.GetSelectedCards`) and bundle screen stored a prompt's TCS in the field the command
  loop polls, then logged and returned the field's task. On a loaded box the command loop could see the prompt, answer it (clearing the
  field) and the game thread then read null: the event option's task faulted (`NullReferenceException`), which nothing reported, and the
  trace showed the event's page as if the answer were lost (Self Help Book, Brain Leech; ~1 run in 650 at 32 processes on 16 cores).
  `setup.sh` publishes the prompt last, from a local, and a faulted event, rest site or shop task is now a command error. (RF2)
- The game's TestMode `RewardsSet.Offer` throws when a reward cannot be taken (a potion on a full belt) and leaves the
  event or purchase hanging. `tracer/Rewards.cs` installs the game's `RewardsSet.testSelector`: it takes the rewards that
  fit and then `SkipLocalRewardsSet`s the rest, which is what a player leaving the rewards screen does. 56 run loadouts
  had been passing vacuously with the throw ignored; they now compare the real skip (RF2 follow-up).
- Rest sites and finished events are forced back to the map (`ForceToMap`) after the choice; the shop is left with `leave_room`. These skip animation steps, not game logic.
- Buying a potion swallows exceptions (a failed buy returns the shop unchanged). Check `potions`/`gold` in the next line.
- sts2-cli sets `TestMode.IsOn`, so merchant potion prices are not randomized (no `shops` draw), and the card removal's `used` never turns true (only the merchant UI node sets it).

## Trace format

One line per step (step 0 is the state at the start of the fight):
`{"step", "action", "decision", "state"}`. `state` holds `combat_over`, `player_dead`, `round`, `side`,
`player {hp,max_hp,block,energy,stars,base_orb_slots,orb_slots,orbs,gold,powers,potions}` (`potions` is the slot list, `null` for an empty slot), `osty {alive,hp,max_hp,block,powers}`,
`rng.run`/`rng.player` (call count of every `RunRngSet`/`PlayerRngSet` stream, as in `dump_run`),
`hand/draw/discard/exhaust/play` (arrays of `{id, upgrade, cost, keywords, exhaust_on_next_play, retain, sly}`: `cost` is `GetAmountToSpend`, `keywords` the sorted
`CardModel.Keywords` names without `None`, `retain`/`sly` are `ShouldRetainThisTurn`/`IsSlyThisTurn`,
`vars` the card's `DynamicVars` in their `CanonicalVars` order as `[name, BaseValue]`, with a third element `Calculate(null)` for a
`CalculatedVar` (a decimal is a JSON integer when integral, else its normalized string; `PreviewValue` is UI-only and not dumped),
and `replay` the `BaseReplayCount`) and
`enemies [{id,alive,hp,max_hp,block,next_move,powers}]`; powers are `{id, amount}`.
`stars` is `PlayerCombatState.Stars` (0 for every character but the Regent at combat start, 3 with `DivineRight`); `base_orb_slots` is `Player.BaseOrbSlotCount`
(Defect 3, persistent), `orb_slots` the combat's `OrbQueue.Capacity` and `orbs` the queue oldest first, each `{id, passive, evoke}` with `id` e.g. `LIGHTNING_ORB` and
`passive`/`evoke` the computed `PassiveVal`/`EvokeVal` (Focus applied; Dark's evoke grows, Glass's passive decays; Plasma passive is its energy). They are always present
in a combat dump (`orb_slots 0`, `orbs []` for the other characters). `osty` is dumped for every character and `{alive: false}` when there is no Osty.
Pile arrays are in the game's list order and **index 0 of `draw` is the top** (the next card drawn).
Since XT3 the dump also carries the per-instance state the game keeps beyond those fields:
- a card's `star_cost` (`GetStarCostWithModifiers`), `last_stars` (`LastStarsSpent`), `clone` (`IsClone`), `affliction` (`{id, amount}`, only when afflicted),
  the enchantment's `status` and `fields`, and the card's own `fields`;
- a power's `amount_on_turn_start`, `skip_tick` (`SkipNextDurationTick`), `applier` and `target` (a creature as `player`, `pet:<id>` or `enemy:<index in enemies>`),
  `vars` (the `DynamicVars` whose `BaseValue` differs from the canonical instance's, only when one does) and `fields`;
- the player's `max_energy`, `turn_number`, `pets` (ids) and `relics` (`{id, vars, fields}`);
- an enemy's `spawned_this_turn`, `performed_first_move` and `move_log` (the move state machine's `StateLog` ids) and `intents`
  (the next move's `AttackIntent`s: `{type, damage, hits}`, `damage` = `GetSingleDamage` against the player, `hits` = `Repeats` of a multi-attack, else 1).
`fields` (only when the model class declares any) are the model subclass's own instance fields, plus a power's `_internalData`'s, as `[name, value]`
sorted by the snake_case field name: numbers, bools and enums as themselves, a card/power/relic as its id, a creature as above, a collection as its count,
any other object as `true` when set, `null` when not. UI state (strings, floats, Godot types, hover tips, `_isActivating`, test/mock fields) is left out.
`dump_run` adds `relic_state` (the relics as above), `removals_used` (`ExtraFields.CardShopRemovalsUsed`) and, on every card, `enchantment_amount` and `fields`.
The comparison (`crates/sts2-diff/src/compare.rs`) is strict on these optional keys: one the referee lists and the engine does not is a divergence.
Engine gaps it leaves out on purpose are its `KNOWN_GAPS` (one model id and key each, with a `TODO`).
Not dumped (XT3 audit): `IsDupe` and `FloorAddedToDeck` (the engine keeps neither), the temporary energy/star cost stacks (their result is
`cost`/`star_cost`), relic `Status`/`IsUsedUp`/`IsWax`/`IsMelted`/`StackCount` (UI or derived from `fields`), non-attack intents (no numbers),
`ExtraPlayerFields` statistics (`DamageDealt`, `DebuffsApplied`, `WongoPoints`), `ExtraRunFields` and the `Discovered*` lists (unlock bookkeeping).
Step 0 also carries `first_encounters`, the seed's pre-rolled normal fights. `error` appears on a failed step.
State comes from our read-only `tracer/TraceDump.cs` (added to the vendored project by `setup.sh` as a
`dump_state` command), not from sts2-cli's own state JSON, which is shaped for agents rather than parity checks.

## Findings

- **Forcing an encounter works**: `enter_room {type: combat, encounter: FUZZY_WURM_CRAWLER_WEAK}`. Seed
  search is not needed, but `find_seed.sh` is provided for the natural path. Seed **`T3S0`** has
  `FuzzyWurmCrawlerWeak` as its natural first Act 1 fight (Overgrowth).
- Forced and natural entry give the same shuffle, hand and monster HP, as long as the Neow choice leaves the deck unchanged
  (checked with a Neow option that only upgrades a card: only the `upgrade` fields differ). Neow choices that add cards change the
  deck and so the shuffle. Neow's options themselves depend on the seed.
- One process, many seeds: `tools/diff.sh` and `tools/diff_run.sh` keep one `referee.py serve` for all their seeds
  (`client.sh`; `REFEREE_FRESH=1` goes back to `run.sh` per seed). Before each seed the `reset` command (`tracer/Reset.cs`)
  ends the run with the game's own `RunManager.CleanUp`, cancels a choice the last seed left open and waits for the
  last combat's turn loop to finish (T14: a turn-start prompt such as Entropy's parked it, and the next fight's loop
  waited 10 s for it, so the fight read as won, "could not reach combat ... card_reward"), replaces sts2-cli's
  `RunSimulator`, clears `CardSelectCmd`'s selector stacks and sets the per-run environment (`STS2_ACT1`, `STS2_REAL_*`);
  its header lists the process-wide leftovers and why none is observable. A turn loop still alive after 5 s makes the
  reset report `stale_turn_loop` and `referee.py` replaces the process. A failed seed replaces the process. Without the reset a second `start_run`
  fails (`State is already set`; before CleanUp existed it reused the first run's rooms). Per seed the fresh path
  spends ~4-5 s (loaded machine) in the first `start_run` (ModelDb, localisation, JIT), 0.2 s starting dotnet and
  0.4-0.7 s in the fight (T13 cut the warm cost to ~30 ms per forced fight and ~0.4 s per run-mode seed, see
  `docs/referee-perf.md`); checked byte-identical to fresh on 4 characters, combat and run mode (greedy, shopper with
  the real-game branches, random), shuffled and repeated seeds in one process (T11).
- One process per slot: under `tools/slot.sh` (which sets `SPIREZERO_SLOT_SOCKET=~/.cache/spirezero-slots/<n>.sock`) the
  client hands the jobs to that slot's daemon (`referee.py serve --socket`), started on first use and kept across
  diff.sh/diff_run.sh jobs, so the first `start_run` is paid once per slot, not once per job. The socket is per build:
  `<n>.<ident>.sock`, ident = referee build + `referee.py` hash, which the daemon answers as `hello <ident>`, so
  checkouts with other builds on the same slots never replace each other's daemon. It exits after `REFEREE_IDLE`
  seconds without a client (default 300; ~250 MB each), replaces its game every 500 seeds (it grows ~0.25 MB per run)
  and after a failed seed. A job never fails because of the process's history: one whose game died (a closed pipe, a
  crash, a reset that could not restore it) or that failed on a reused game is replayed once on a fresh game, whose
  result is the answer (`REFEREE_NO_RETRY=1` turns that off); a client whose daemon dies (or closes before its hello,
  the idle exit racing a connect) retries on a new one and then on a game of its own. Before T14 a dead game raised
  `BrokenPipeError` in the daemon, which took it for a lost client and kept the dead game, so every later job of that
  slot failed with "referee daemon died" (1497 of 2932 regress jobs on 2026-10-18). sts2-cli's unhandled-exception
  handler prints a `game_over` line and exits, so a game thread that outlives its run (an event card reward's 300 s
  wait) can kill the process; `reset` now drains that wait. Warm, a seed costs 20-190 ms of `start_run` (room and map generation for every act, seed-dependent) plus
  the fight (~30 ms for a 12-action combat). 6 jobs x 5 seeds: 44 s fresh per seed, 13 s one process per job, 7 s per slot.
- `referee/stress_reset.py [-n 300]` checks the reset: random forced fights (every loadout and character, plain
  encounters) and runs (every policy, run loadouts, the real-game branches) at a0/a10, plus prefixes of them cut on a
  prompt, played fresh and then in shuffled orders on warm processes (with `REFEREE_NO_RETRY=1`); every trace and
  status must be byte-identical. A tenth of the jobs use a loadout with a turn-start prompt (Entropy, Tools of the
  Trade, Gambling Chip, Toolbox), which found the T14 leak (25 differences in 1092 warm runs before the fix, 0 after).
- Same input twice gives byte-identical traces (checked 3x).

## Inputs beyond the seed (all pinned in the harness)

| Input | Pinned to |
|---|---|
| Character, ascension | Necrobinder, 0 (`CHARACTER`/`ASCENSION` in `referee.py`, overridable per actions file) |
| Unlock/epoch state | `UnlockState.all`: every epoch, every encounter seen, `NumberOfRuns = 999999999` (so no first-run overrides). sts2-cli's `CreatePlayer` passes it to `Player.CreateForNewRun` and `RunState.UnlockState` is the merge of the players'; `dump_run` prints `unlock_number_of_runs` on step 0 to confirm |
| Save profile / progress | none read: `SaveManager.InitProfileId(0)` with a fresh in-memory profile; the user's Steam saves are never read or written |
| Game build | v0.111.0 (`commit 41cef1ea`), the installed DLLs |

A real profile differs from this in two ways that matter for encounter selection and event pools: epoch reveals
(`GenerateRooms` filters events by `IsEpochRevealed<EventNEpoch>`) and `NumberOfRuns == 0`, which swaps in a fixed first-run room order
(`Overgrowth.ApplyActDiscoveryOrderModifications`). The harness shows a seed-dependent order (not the fixed first-run one), so it corresponds to an "all unlocked, not first run" profile.
Run history files (`.run`) from the real profile are not directly comparable on encounter order.

## Godot stubs

sts2-cli replaces `GodotSharp.dll` with stubs. A game method that references a Godot member the stubs lack fails to JIT and throws
`MissingMethodException` as a whole when called, even when the reference is in a branch the headless run never takes, and the game
often swallows it (a power hook just does nothing). Before T17 this lost PhantasmalGardener's Enlarge (`Mathf.Log`), Rolling Boulder's
damage (`GodotObject.Connect` in its `TestMode.IsOff` branch), the Kaiser Crab's `AfterAddedToRoom`, `GameAction.Cancel` and Crystal Sphere's gold item.
`setup.sh` replaces the stub `Mathf` with `referee/stubs/Mathf.cs` (every member `sts2.dll` references, with GodotSharp 4.5's float/double
semantics) and adds the other members in `referee/stubs/GodotStubsExtra.cs`. After the build, `referee/stubcheck` scans the patched
`sts2.dll` and fails the build on any method of a game-logic namespace (`Models`, `Commands`, `Combat`, `Entities`, `Rewards`, ...) that
references a missing Godot type or member, and on any missing `Mathf` member anywhere, except the unreachable ones listed with a reason
in `referee/stubcheck/allowed.txt` (UI icons, multiplayer, empty achievement checks). UI code (`Nodes`, `RichTextTags`, ...) is not checked.

## Node singletons (XT4)

Headless there is no scene tree, so every Godot node singleton (`NCombatRoom.Instance`, `NAudioManager.Instance`, `NRun.Instance`, ...)
is null, and game logic that dereferences one without `?.` threw: Kaiser Crab arms' `BeforeDeath` (`NAudioManager.Instance.PlayOneShot`)
threw before `AfterDeath`, so CrabRage never fired, and `SoulNexus.AfterDeath` (`NCombatRoom.Instance.GetCreatureNode`) stalled the kill.
sts2-cli's own guard covers only a name allowlist in async state machines. After the build, `referee/nullguard` (Mono.Cecil) rewrites every
member access on a node singleton in a game-logic namespace (the same list as stubcheck) into a generated helper
(`MegaCrit.Sts2.Core.Referee.NullGuard`) that returns the default for a null receiver and is the original access otherwise, following node
results (`NCombatRoom.Instance.Ui.Hand`); about 240 sites. With a non-null receiver nothing changes, so game rules are untouched. A read it
does not rewrite (stored in a local, passed as an argument, a non-node result) must be listed with a reason in `referee/nullguard/escapes.txt`
(each one is null-checked or behind `TestMode.IsOn`); the build fails on a new one, on a stale entry and on an access it cannot rewrite.

## Caveat: IL patches

sts2-cli patches `sts2.dll` in memory/on its copy (headless async plumbing: `Task.Yield`, `Cmd.Wait`, the action-queue wait, null
guards on UI/VFX singletons, `TalkCmd`, asset-cache casts; the null guards broke two methods' branches until XT2, see `docs/semantics/referee.md`, which
made THE_INSATIABLE_BOSS stall on its first `end_turn`) plus Harmony patches for localisation and rewards (the Harmony ones fail at start-up: the process runs on .NET 10,
which Harmony does not support; `docs/referee-perf.md`). They target UI and timing, not game
rules; the installed DLLs are untouched. Anything that behaves oddly around animations or UI waits should be checked against the decomp.
