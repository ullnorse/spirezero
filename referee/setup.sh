#!/usr/bin/env bash
# Build the referee: fetch sts2-cli at a pinned commit, copy the installed game DLLs into its lib/,
# apply its headless IL patches, add our tracer sources, and build.
#
# Builds are immutable and shared: the result lives in referee/builds/<hash of everything the build reads>/ and
# is never rebuilt in place. The hash covers this script, every file of the source folders it copies (tracer,
# patcher, stubs, stubcheck, nullguard), the sts2-cli pin, the installed game's DLLs and the .NET SDK version, so a
# Steam update or a new SDK makes a new build instead of reusing the old one. The game must be v0.111.0 (ref/ and
# the engine target it). A `current` symlink in referee/builds/ is switched
# atomically to it, so runs that already started (they resolve the build once, see referee/run.sh)
# and runs from other worktrees are not disturbed. Rerunning with unchanged sources only
# re-points `current`. Everything it creates is gitignored.
set -euo pipefail
export DOTNET_ROOT=${DOTNET_ROOT:-$HOME/.dotnet} PATH=$HOME/.dotnet:$HOME/.dotnet/tools:$PATH
here=$(cd "$(dirname "$0")" && pwd)
pin=7662fdcebe152f475e69c133b6eefbcf256f09fb
game=${STS2_GAME:-$HOME/.local/share/Steam/steamapps/common/Slay the Spire 2/data_sts2_linuxbsd_x86_64}
builds=$here/builds
mkdir -p "$builds"

grep -q '"version": "v0.111.0"' "$game/../release_info.json" || { echo "setup.sh: the game at $game is not v0.111.0" >&2; exit 1; }
inputs() {
    echo "pin $pin"
    echo "dotnet $(dotnet --version)"
    (cd "$here" && find setup.sh tracer patcher stubs stubcheck nullguard -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum)
    (cd "$game" && find . -maxdepth 1 -name '*.dll' -print0 | LC_ALL=C sort -z | xargs -0 sha256sum)
}
hash=$(inputs | sha256sum | cut -c1-12)
final=$builds/$hash
if [ ! -f "$final/.complete" ]; then
    # The pristine upstream clone is only a cache; each build starts from a fresh copy of it.
    upstream=$here/vendor/upstream
    if [ ! -d "$upstream/.git" ]; then
        mkdir -p "$here/vendor"
        git clone --quiet https://github.com/morefreeze/sts2-cli "$upstream"
    fi
    stage=$(mktemp -d "$builds/.stage.XXXXXX")
    chmod 755 "$stage"
    trap 'rm -rf "$stage"' EXIT
    vendor=$stage/sts2-cli
    git clone --quiet --no-checkout "$upstream" "$vendor"
    git -C "$vendor" checkout --quiet "$pin"
    # vendor/setup.sh minus its trailing Python smoke test (we have our own).
    sed '/^echo "🩺 Smoke test/,$d' "$vendor/setup.sh" > "$vendor/.setup_nosmoke.sh"
    # sts2-cli's IL patch 4 (null guards on singletons in state machines) inserts instructions with Mono.Cecil but keeps
    # the method's short branches, and Cecil writes a short branch whose target moved out of its signed-byte range
    # with the offset wrapped, so the method is invalid IL and throws InvalidProgramException as a whole when JIT
    # compiled, whatever branch it would take. SandpitPower.UpdateCreaturePositions (whose TestMode.IsOn branch only
    # returns) had a `brfalse.s` wrap backwards: THE_INSATIABLE_BOSS's turn loop died at the first enemy turn end (XT2).
    # Widen every branch of a method before patching it (SimplifyMacros) and pick the short forms again after.
    perl -0pi -e 's{^using Mono\.Cecil\.Cil;\n}{$&using Mono.Cecil.Rocks;\n}m; s{^( +)bool restart = true;\n}{$1method.Body.SimplifyMacros();\n$&}m; s{(\n {24}restart = true;\n {24}break;\n {20}\}\n {16}\}\n)}{$1                method.Body.OptimizeMacros();\n}' "$vendor/.setup_nosmoke.sh"
    [ "$(grep -c 'using Mono.Cecil.Rocks;\|method.Body.SimplifyMacros();\|method.Body.OptimizeMacros();' "$vendor/.setup_nosmoke.sh")" = 3 ]
    log=$stage/setup.log  # quiet unless a step fails
    trap 'cat "$log" >&2; rm -rf "$stage"' ERR
    (cd "$vendor" && bash .setup_nosmoke.sh "$game") > "$log" 2>&1
    rm -f "$vendor/.setup_nosmoke.sh"

    # Add our commands to the protocol: read-only `dump_state` (referee/tracer/TraceDump.cs) and
    # `dump_run` (referee/tracer/RunDump.cs), `set_loadout` (referee/tracer/Loadout.cs) and
    # run mode's `run_loadout` (referee/tracer/RunLoadout.cs, a partial of RunSimulator for its private fields), and
    # `reset` (referee/tracer/Reset.cs), which ends the run and replaces the simulator so the process can start another.
    cp "$here/tracer/TraceDump.cs" "$here/tracer/RunDump.cs" "$here/tracer/Loadout.cs" "$here/tracer/RunLoadout.cs" \
        "$here/tracer/Reset.cs" "$here/tracer/RunWaits.cs" "$here/tracer/RunEnd.cs" "$here/tracer/NoHarmony.cs" "$here/tracer/CardAlternative.cs" \
        "$here/tracer/Rewards.cs" "$vendor/src/Sts2Headless/"
    prog=$vendor/src/Sts2Headless/Program.cs
    if ! grep -q dump_state "$prog"; then
        sed -i 's|^            case "quit":|            case "dump_state":\n                return TraceDump.Dump();\n\n            case "quit":|' "$prog"
    fi
    if ! grep -q set_loadout "$prog"; then
        sed -i 's|^            case "quit":|            case "set_loadout":\n                return Loadout.Apply(cmd);\n\n            case "quit":|' "$prog"
    fi
    if ! grep -q run_loadout "$prog"; then
        sed -i 's|^            case "quit":|            case "run_loadout":\n                return sim.ApplyRunLoadout(cmd);\n\n            case "quit":|' "$prog"
    fi
    if ! grep -q dump_run "$prog"; then
        sed -i 's|^            case "quit":|            case "dump_run":\n                return RunDump.Dump(sim, cmd.TryGetProperty("map", out var withMap) \&\& withMap.GetBoolean(),\n                    cmd.TryGetProperty("act_maps", out var withActMaps) \&\& withActMaps.GetBoolean());\n\n            case "quit":|' "$prog"
    fi
    # A forced fight never reads the act map, so start_run's `forced_fight` skips its pruning (referee/patcher).
    if ! grep -q STS2_FORCED_FIGHT "$prog"; then
        sed -i 's|^\( *\)case "start_run":|&\n\1    Environment.SetEnvironmentVariable("STS2_FORCED_FIGHT", cmd.TryGetProperty("forced_fight", out var forced) \&\& forced.GetBoolean() ? "1" : null);|' "$prog"
    fi
    if ! grep -q RefereeReset "$prog"; then
        sed -i 's|^\( *\)result = HandleCommand(sim, cmd);|\1result = cmd.GetProperty("cmd").GetString() == "reset" ? RefereeReset.Reset(ref sim, cmd) : HandleCommand(sim, cmd);|' "$prog"
    fi
    # sts2-cli's HealBetweenActs is not in the real game (the next act's Ancient event does the heal), so
    # its two call sites are turned off. See referee/README.md "Where sts2-cli is not the real game".
    rs=$vendor/src/Sts2Headless/RunSimulator.cs
    sed -i 's|^public class RunSimulator|public partial class RunSimulator|' "$rs"
    sed -i 's|^\( *\)HealBetweenActs();|\1// referee: HealBetweenActs() disabled, not in the real game|' "$rs"
    # sts2-cli generates a won fight's room rewards itself, ignoring `EncounterModel.ShouldGiveRewards`; the game
    # offers none for such an encounter (`NCombatUi.OnCombatWon` -> `ProceedWithoutRewards`, the BattlewornDummy
    # fights). See referee/README.md "Where sts2-cli is not the real game".
    # Without the rewards nothing pumps the combat's end to completion first (referee/tracer/RunWaits.cs WaitForCombatEnd).
    sed -i 's|^\( *\)if (_pendingRewards == null && !_rewardsProcessed)$|\1if (combatRoom.Encounter?.ShouldGiveRewards == false) WaitForCombatEnd(combatRoom);\n\1if (_pendingRewards == null \&\& !_rewardsProcessed \&\& combatRoom.Encounter?.ShouldGiveRewards != false)|' "$rs"
    grep -q 'ShouldGiveRewards != false' "$rs"
    # sts2-cli's runtime Harmony patches (Task.Yield, Cmd.Wait, TalkCmd, RewardsSet.Offer, FromChooseABundleScreen, the
    # LocTable fallbacks, ...) only applied when the process ran on .NET 9; on .NET 10 they all failed. The referee is
    # the game without them on any runtime (referee/tracer/NoHarmony.cs, PX16).
    sed -i 's|new Harmony("|RefereeNoHarmony.Create("|' "$rs"
    [ "$(grep -c 'new Harmony(' "$rs")" = 0 ] && [ "$(grep -c 'RefereeNoHarmony.Create("' "$rs")" = 8 ]
    # The real game canonicalises a typed seed before the run hashes it (StartRunLobby.BeginRunForAllPlayersIfAllReady:
    # SeedHelper.CanonicalizeSeed, upper-case, O -> 0, I -> 1, trim); sts2-cli passed it raw, so seeds with O, I or
    # lower case diverged from the engine (docs/semantics/run.md).
    sed -i 's|^\( *\)var seedStr = seed ?? |\1var seedStr = seed != null ? MegaCrit.Sts2.Core.Helpers.SeedHelper.CanonicalizeSeed(seed) : |' "$rs"
    grep -q "SeedHelper.CanonicalizeSeed(seed)" "$rs"
    # Run mode can choose the first act: with STS2_ACT1=underdocks (set by referee.py from the actions file's
    # "act1") the run's acts are Underdocks, Hive, Glory; otherwise null falls back to the default list
    # (Overgrowth, Hive, Glory). sts2-cli never goes through ActModel.GetRandomList, see docs/specs/underdocks.md.
    if ! grep -q "STS2_ACT1" "$rs"; then
        sed -i 's|^\( *\)players: new\[\] { player },|&\n\1acts: Environment.GetEnvironmentVariable("STS2_ACT1") == "underdocks" ? new MegaCrit.Sts2.Core.Models.ActModel[] { MegaCrit.Sts2.Core.Models.ModelDb.Act<MegaCrit.Sts2.Core.Models.Acts.Underdocks>(), MegaCrit.Sts2.Core.Models.ModelDb.Act<MegaCrit.Sts2.Core.Models.Acts.Hive>(), MegaCrit.Sts2.Core.Models.ModelDb.Act<MegaCrit.Sts2.Core.Models.Acts.Glory>() } : null,|' "$rs"
    fi
    # An event option that asks for cards (Neow's Pomander, Precise Scissors...) runs on a background task;
    # sts2-cli returns from select_cards before that task has applied the pick and finished the event, so the
    # trace line would show the event still open and the deck unchanged. Wait for the task first, like
    # sts2-cli already does for the rest site's Smith.
    if ! grep -q "referee: event card selection" "$rs"; then
        sed -i 's|^        // Extra wait for shop card removal: the purchase task needs to finish|        // referee: event card selection, wait for the option task to finish\n        WaitForLoadoutTask();\n        if (_runState?.CurrentRoom is EventRoom)\n        {\n            for (int i = 0; i < 2000 \&\& !_cardSelector.HasPending \&\& RunManager.Instance.EventSynchronizer?.GetLocalEvent() is { IsFinished: false }; i++)\n            {\n                _syncCtx.Pump();\n                Thread.Sleep(1);\n            }\n            _syncCtx.Pump();\n            WaitForActionExecutor();\n        }\n\n&|' "$rs"
    fi
    # An event's card reward (Brain Leech RIP, The Future of Potions) is a RewardsSet that sts2-cli answers through
    # the game's blocking card choice. Skipping it leaves the set incomplete, so RewardsCmd.OfferCustom and the event
    # never finish (in TestMode `Offer` throws once the choice returns untaken); the game's Proceed button skips the
    # set (NRewardsScreen: SkipLocalRewardsSet). Do that before answering the choice, and after a pick or a skip wait
    # for the option task like for select_cards. The wait ends once the answered reward is taken (the game may offer the
    # next one at once, so "no reward pending" can stay false until the timeout).
    if ! grep -q "referee: event card reward" "$rs"; then
        sed -i 's|^    private void WaitForActionExecutor()|    // referee: event card reward, wait for the option task to finish or prompt again\n    private void WaitForEventTask(object? answered)\n    {\n        WaitForLoadoutTask();\n        for (int i = 0; i < 2000 \&\& _cardSelector.HasPendingReward \&\& ReferenceEquals(_cardSelector.PendingRewardCards, answered); i++) Thread.Sleep(1);\n        for (int i = 0; i < 2000 \&\& !_cardSelector.HasPending \&\& !_cardSelector.HasPendingReward \&\& RunManager.Instance.EventSynchronizer?.GetLocalEvent() is { IsFinished: false }; i++)\n        {\n            _syncCtx.Pump();\n            Thread.Sleep(1);\n        }\n        _syncCtx.Pump();\n        WaitForActionExecutor();\n    }\n\n&|' "$rs"
        sed -i 's|^\( *\)_cardSelector.ResolveReward(idx);|\1var answered = _cardSelector.PendingRewardCards;\n&\n\1WaitForEventTask(answered);|' "$rs"
        sed -i 's|^\( *\)_cardSelector.SkipReward();|\1RunManager.Instance.RewardsSetSynchronizer.SkipLocalRewardsSet();\n\1var answered = _cardSelector.PendingRewardCards;\n&\n\1WaitForEventTask(answered);|' "$rs"
    fi
    # A forced fight (start_run's `forced_fight`, STS2_FORCED_FIGHT) is diffed as a fight only, so its trace ends on the
    # state the fight left. sts2-cli claims a won fight's gold, relic and potion rewards before showing the card reward,
    # and a claimed relic's AfterObtained changes that state (Whetstone's StableShuffle draws Niche, Strawberry raises
    # max HP). In a forced fight the rewards are still generated but not claimed; run mode is unchanged.
    perl -0pi -e 's{if \(reward is GoldReward \|\| reward is MegaCrit\.Sts2\.Core\.Rewards\.RelicReward\n(\s*)\|\| reward is MegaCrit\.Sts2\.Core\.Rewards\.PotionReward\)}{if (Environment.GetEnvironmentVariable("STS2_FORCED_FIGHT") != "1" && (reward is GoldReward || reward is MegaCrit.Sts2.Core.Rewards.RelicReward\n$1|| reward is MegaCrit.Sts2.Core.Rewards.PotionReward))}' "$rs"
    [ "$(grep -c 'GetEnvironmentVariable("STS2_FORCED_FIGHT") != "1" && (reward is GoldReward' "$rs")" = 1 ]
    # Speed (docs/referee-perf.md). The loops that wait for a background task (an event option, a rest site option,
    # a card or reward pick) poll every 1 ms instead of every 10 ms, with ten times the iterations, so each one ends
    # as soon as its condition holds and still times out at the same moment. Only the waits change, not their conditions.
    perl -0pi -e 's{for \(int i = 0; i < (\d+)((?:(?!\bfor\b|\bwhile\b)[^;{}])*); i\+\+\)(\s*(?:\{(?:(?!\bfor\b|\bwhile\b)[^{}])*?)?)Thread\.Sleep\(10\);}{"for (int i = 0; i < ".($1*10).$2."; i++)".$3."Thread.Sleep(1);"}ge' "$rs"
    # Dynamic PGO makes the warm process about 2x slower once start_run stops dominating (the combat code is tiered
    # with a profile that does not fit the next fights); it changes code generation, not results.
    sed -i 's|^\( *\)<RollForward>LatestMajor</RollForward>|&\n\1<TieredPGO>false</TieredPGO>|' "$vendor/src/Sts2Headless/Sts2Headless.csproj"
    # The rest site's option task is kept (referee/tracer/RunWaits.cs): its two fixed 200 ms waits, after a non-Smith
    # option and after Smith's card pick, wait for that task to finish instead.
    sed -i 's|^\( *\)var task = Task.Run(() => RunManager.Instance.RestSiteSynchronizer.ChooseLocalOption(optionIndex));|\1var task = _restTask = Task.Run(() => RunManager.Instance.RestSiteSynchronizer.ChooseLocalOption(optionIndex));|' "$rs"
    perl -0pi -e 's{(// Give the action time to complete \(heal HP, dig for relic, etc\.\)\n(?:.*\n){2}\s*)Thread\.Sleep\(200\);}{$1WaitForRestTask();}; s{(if \(_runState\?\.CurrentRoom is RestSiteRoom\)\n\s*\{\n\s*)Thread\.Sleep\(200\);}{$1WaitForRestTask();}' "$rs"
    [ "$(grep -c 'WaitForRestTask();\|_restTask = Task.Run' "$rs")" = 3 ]
    # A rest-site heal's rewards (DreamCatcher's card reward, RewardsCmd.OfferCustom) block the option task on the game's
    # card choice (HasPendingReward), which sts2-cli's rest site did not wait for: it forced the map and dropped them.
    # Stop on that choice like an event's and show it; once it is answered (and no other prompt follows), leave the
    # rest site as after any finished option (referee/tracer/RunWaits.cs AfterRewardAnswered).
    perl -0pi -e 's{(_restTask = Task\.Run\(.*\n(?:.*\n){3}\s*if \(_cardSelector\.HasPending)\) break;((?:.*\n){4}\s*if \(_cardSelector\.HasPending)\)}{$1 || _cardSelector.HasPendingReward) break;$2 || _cardSelector.HasPendingReward)}' "$rs"
    perl -0pi -e 's{(WaitForEventTask\(answered\);\n\s*Thread\.Sleep\(50\);\n\s*_syncCtx\.Pump\(\);\n\s*WaitForActionExecutor\(\);\n\s*return )DetectDecisionPoint\(\);}{$1AfterRewardAnswered();}g' "$rs"
    [ "$(grep -c '_cardSelector.HasPending || _cardSelector.HasPendingReward) break;' "$rs")" = 3 ]
    [ "$(grep -c 'return AfterRewardAnswered();' "$rs")" = 2 ]
    # Combat waits (referee/tracer/RunWaits.cs, T18): a command returns once the game has settled or reports a timeout.
    # Ours replace sts2-cli's ExecuteAction (wrapped), DoEndTurn and WaitForActionExecutor, and the end of
    # DetectDecisionPoint's combat branch, which reported combat_play after a fixed 100 ms whatever the state, and its
    # game over on death, which came before the game had ended the combat.
    sed -i 's|^    public Dictionary<string, object?> ExecuteAction(string action|    private Dictionary<string, object?> ExecuteActionUpstream(string action|; s|^    private Dictionary<string, object?> DoEndTurn(Player player)|    private Dictionary<string, object?> DoEndTurnUpstream(Player player)|; s|^    private void WaitForActionExecutor()|    private void WaitForActionExecutorUpstream()|' "$rs"
    perl -0pi -e 's{\n(\s*)if \(CombatManager\.Instance\.IsInProgress && IsPlayPhase\(\)\)\n\s*\{\n\s*return CombatPlayState\(player\);\n(?:(?!\n\s*// Event room).)*?\n\s*if \(HasSurfaceablePrompt\) goto recheckPending;\n\s*return CombatPlayState\(player\);}{\n$1var settled = SettledCombatDecision(player, combatRoom);\n$1if (settled == null) goto recheckPending;\n$1return settled;}s' "$rs"
    perl -0pi -e 's{(// Check game over \(death\)\n\s*if \(player\.Creature != null && player\.Creature\.IsDead\)\n\s*\{\n\s*return )GameOverState\(false\);}{$1SettledDeath();}' "$rs"
    [ "$(grep -c 'ExecuteActionUpstream(string action\|DoEndTurnUpstream(Player player)\|void WaitForActionExecutorUpstream()\|var settled = SettledCombatDecision\|return SettledDeath();' "$rs")" = 5 ]
    # sts2-cli reported victory at the first kill of any last-act boss (its Proceed and its post-reward path), including
    # the first of A10's two; the game goes on to the second boss and wins only in TheArchitect's room
    # (referee/tracer/RunEnd.cs AfterBoss).
    perl -0pi -e 's{if \(_runState != null && _runState\.CurrentActIndex >= 2\)\n\s*\{\n\s*Log\(\$"Final boss defeated[^\n]*\n\s*return GameOverState\(true\);\n(\s*)\}}{if (AfterBoss() is { } afterBoss)\n$1    return afterBoss;}g' "$rs"
    [ "$(grep -c 'if (AfterBoss() is { } afterBoss)' "$rs")" = 2 ]
    # Run-mode background tasks (referee/tracer/RunWaits.cs WaitForGameTask, PX15): an event option's, a shop purchase's or
    # removal's task is kept, and the waits that gave up after a fixed time (`task.Wait(2000)`, 2000 polls after an event's
    # card pick or reward, a fixed 200 ms after a shop removal's pick) wait until the task is done, the event finished or
    # a prompt is open, the conditions they had, and report a timeout instead of dumping a half-applied state.
    sed -i 's|^\( *\)var task = Task.Run(() => options\[optionIndex\].Chosen());|\1var task = _eventTask = Task.Run(() => options[optionIndex].Chosen());|; s|^\( *\)var task = Task.Run(() => entry.OnTryPurchaseWrapper(inv));|\1var task = _shopTask = Task.Run(() => entry.OnTryPurchaseWrapper(inv));|; s|^\( *\)var task = Task.Run(() => removal.OnTryPurchaseWrapper(|\1var task = _shopTask = Task.Run(() => removal.OnTryPurchaseWrapper(|; s|^\( *\)if (!task.IsCompleted) task.Wait(2000);|\1WaitForGameTask(task);|' "$rs"
    perl -0pi -e 's{for \(int i = 0; i < 2000 && _cardSelector\.HasPendingReward && ReferenceEquals\(_cardSelector\.PendingRewardCards, answered\); i\+\+\) Thread\.Sleep\(1\);}{if (!PumpUntil(() => !_cardSelector.HasPendingReward || !ReferenceEquals(_cardSelector.PendingRewardCards, answered))) _settleError ??= "the card reward was not taken";}; s{for \(int i = 0; i < 2000 && !_cardSelector\.HasPending(?: && !_cardSelector\.HasPendingReward)? && RunManager\.Instance\.EventSynchronizer\?\.GetLocalEvent\(\) is \{ IsFinished: false \}; i\+\+\)\n\s*\{\n\s*_syncCtx\.Pump\(\);\n\s*Thread\.Sleep\(1\);\n\s*\}}{WaitForEventOption();}g; s{(if \(_runState\?\.CurrentRoom is MerchantRoom\)\n\s*\{\n\s*)Thread\.Sleep\(200\);}{$1WaitForGameTask(_shopTask);}' "$rs"
    [ "$(grep -c '_eventTask = Task.Run\|_shopTask = Task.Run\|WaitForGameTask(task);\|WaitForEventOption();\|WaitForGameTask(_shopTask);\|"the card reward was not taken"' "$rs")" = 11 ]
    [ "$(grep -c 'task.Wait(2000)' "$rs")" = 0 ]
    # Room-entry purchases (PX18, referee/tracer/RunWaits.cs): LordsParasol buys everything from the room entry and on in the
    # continuation of each prompt's answer, on the command's thread, where Orrery's blocking card reward hangs it for good.
    # The entry of a possible shop with the relic, and a shop's card selection answer, run on a task like a player's own
    # purchase; a shop's card reward answer then waits for the interrupted purchases.
    sed -i 's|^\( *\)RunManager.Instance.EnterMapCoord(coord).GetAwaiter().GetResult();|\1EnterMapCoordOnTask(coord);|' "$rs"
    sed -i 's|^\( *\)_cardSelector.ResolvePendingByIndices(indices);|\1ResolvePendingOffThread(indices);|' "$rs"
    perl -0pi -e 's{("the card reward was not taken";\n\s*WaitForEventOption\(\);\n)}{$1        WaitForShopTasks();\n}' "$rs"
    [ "$(grep -c 'EnterMapCoordOnTask(coord);\|ResolvePendingOffThread(indices);\|        WaitForShopTasks();' "$rs")" = 3 ]
    # An event fight's resume (RF1, referee/tracer/RunWaits.cs ResumeEventOnTask): leaving a won BattlewornDummy fight
    # resumes the event, whose card reward (TheHunt's) blocked the command's thread; it runs on a task and is shown.
    perl -0pi -e 's{(// Normal → go to map\n(\s*))ForceToMap\(\);\n}{$1if (ResumeEventOnTask(combatRoom)) return DetectDecisionPoint();\n$2ForceToMap();\n}' "$rs"
    [ "$(grep -c 'if (ResumeEventOnTask(combatRoom)) return DetectDecisionPoint();' "$rs")" = 1 ]
    # A prompt is published last (RF2): the test selector's GetSelectedCards and the bundle screen stored their TCS in the
    # field the main loop reads, then logged and returned the field's task. Under load the main loop could see the
    # prompt, answer it (which clears the field) and the game thread then read null: its option task faulted with a
    # NullReferenceException and the event stayed on its page as if never answered (SG2 group G, Self Help Book).
    # The task is built in a local and the field is written (Volatile.Write) after everything the main loop reads.
    perl -0pi -e 's{_pendingTcs = new TaskCompletionSource<IEnumerable<CardModel>>\(\);\n\n(\s*Console\.Error\.WriteLine\(\$"\[SIM\] Card selection pending:[^\n]*\n)\n\s*// Return the task[^\n]*\n(\s*)return _pendingTcs\.Task;}{var tcs = new TaskCompletionSource<IEnumerable<CardModel>>();\n$1$2Volatile.Write(ref _pendingTcs, tcs);\n$2return tcs.Task;}; s{(\s*)sim\._pendingBundles = bundles;\n\s*sim\._pendingBundleTcs = new TaskCompletionSource<IEnumerable<CardModel>>\(\);\n(\s*Console\.Error\.WriteLine\(\$"\[SIM\] Bundle selection pending:[^\n]*\n)\n\s*__result = sim\._pendingBundleTcs\.Task;}{$1var tcs = new TaskCompletionSource<IEnumerable<CardModel>>();\n$2$1Volatile.Write(ref sim._pendingBundleTcs, tcs);$1Volatile.Write(ref sim._pendingBundles, bundles);$1__result = tcs.Task;}' "$rs"
    [ "$(grep -c 'Volatile.Write(ref _pendingTcs, tcs);\|Volatile.Write(ref sim._pendingBundleTcs, tcs);\|Volatile.Write(ref sim._pendingBundles, bundles);' "$rs")" = 3 ]
    [ "$(grep -c 'return _pendingTcs.Task;\|__result = sim._pendingBundleTcs.Task;' "$rs")" = 0 ]
    # A card reward's alternatives (Driftwood's REROLL, PaelsWing's SACRIFICE, referee/tracer/CardAlternative.cs, PX21): the
    # `card_alternative` action, and the test selector's answer to an event's card reward with an alternative instead of a
    # card (`GetSelectedCardReward` returned only a card or a skip; `CardReward.OnSelect` takes the alternative from there).
    sed -i 's|^\( *\)case "skip_card_reward":|\1case "card_alternative":\n\1    return DoCardAlternative(player, args);\n&|' "$rs"
    perl -0pi -e 's{(            _rewardChoice = -1;\n)(            _rewardWait = new ManualResetEventSlim\(false\);)}{$1            _rewardAlternative = -1;\n            PendingRewardAlternatives = alternatives.ToList();\n$2}; s{(            _rewardWait = null;\n\n)(            if \(choice >= 0)}{$1            var offered = PendingRewardAlternatives;\n            PendingRewardAlternatives = null;\n            if (_rewardAlternative >= 0 && _rewardAlternative < offered!.Count)\n                return new MegaCrit.Sts2.Core.TestSupport.CardRewardSelection { alternative = offered[_rewardAlternative] };\n\n$2}; s{(        public bool HasPendingReward =>)}{        public List<CardRewardAlternative>? PendingRewardAlternatives { get; private set; }\n        private int _rewardAlternative = -1;\n\n        public void ResolveRewardAlternative(int index)\n        {\n            _rewardAlternative = index;\n            _rewardChoice = -1;\n            _rewardWait?.Set();\n        }\n\n$1}' "$vendor/src/Sts2Headless/RunSimulator.cs"
    [ "$(grep -c 'ResolveRewardAlternative\|PendingRewardAlternatives = alternatives.ToList();\|case "card_alternative":' "$rs")" = 3 ]
    # Opt-in real-game branches of the TestMode gameplay sites (referee/patcher, docs/specs/test-mode.md): inert
    # unless referee.py sets STS2_REAL_SHOP / STS2_REAL_CAULDRON / STS2_REAL_BELL.
    cp -r "$here/patcher" "$stage/patcher"
    (cd "$stage/patcher" && dotnet run -v q -- "$vendor/lib/sts2.dll") >> "$log" 2>&1
    rm -rf "$stage/patcher"
    # Godot stub gaps (referee/stubs): our Mathf replaces sts2-cli's (which lacked Log, so PhantasmalGardener's
    # Enlarge threw), and GodotStubsExtra.cs adds members of partial stub types that game logic references.
    stubs=$vendor/src/GodotStubs
    perl -0pi -e 's/^public static class Mathf\n\{\n.*?^\}\n//sm' "$stubs/Math.cs"
    sed -i 's|^public class GodotObject$|public partial class GodotObject|' "$stubs/Core.cs"
    sed -i 's|^public class CanvasItem : Node$|public partial class CanvasItem : Node|' "$stubs/UI.cs"
    sed -i 's|^public struct Vector2I$|public partial struct Vector2I|' "$stubs/Math.cs"
    sed -i 's|^public class SceneTreeTimer : GodotObject$|public partial class SceneTreeTimer : GodotObject|' "$stubs/Core.cs"
    sed -i 's|^public struct Callable$|public partial struct Callable|; s|^\( *\)public void Call(params Variant\[\] args) => (_delegate as Action)?.Invoke();|\1public void Call(params Variant[] args) => InvokeParameterless();|' "$stubs/Types.cs"
    [ "$(grep -c 'class Mathf' "$stubs/Math.cs")" = 0 ]
    [ "$(cat "$stubs"/*.cs | grep -c '^public partial \(class GodotObject\|class CanvasItem\|struct Vector2I\|struct Callable\|class SceneTreeTimer\)\|InvokeParameterless();')" = 6 ]
    cp "$here"/stubs/*.cs "$stubs/"
    dotnet build "$vendor/src/Sts2Headless/Sts2Headless.csproj" -nologo -v q -clp:ErrorsOnly >> "$log" 2>&1
    [ -f "$vendor/src/Sts2Headless/bin/Debug/net9.0/Sts2Headless.dll" ]
    # Null-safe Godot node singletons (referee/nullguard): a game-logic dereference of `NCombatRoom.Instance`,
    # `NAudioManager.Instance`, ... does nothing headless instead of throwing. It needs the built stubs to resolve
    # Godot members, so it patches lib/sts2.dll after the build and replaces the built copy. Fails on an unreviewed read.
    cp -r "$here/nullguard" "$stage/nullguard"
    (cd "$stage/nullguard" && dotnet run -v q -- "$vendor/lib/sts2.dll" "$stubs/bin/Debug/net9.0" escapes.txt) >> "$log" 2>&1
    rm -rf "$stage/nullguard"
    cp "$vendor/lib/sts2.dll" "$vendor/src/Sts2Headless/bin/Debug/net9.0/sts2.dll"
    # Fail on a game-logic method that references a Godot member the stubs lack (referee/stubcheck).
    cp -r "$here/stubcheck" "$stage/stubcheck"
    (cd "$stage/stubcheck" && dotnet run -v q -- "$vendor/lib/sts2.dll" "$stubs/bin/Debug/net9.0" allowed.txt) >> "$log" 2>&1
    rm -rf "$stage/stubcheck"
    rm -f "$log"
    touch "$stage/.complete"
    mv -T "$stage" "$final" 2>/dev/null || true  # a concurrent setup of the same hash won
    trap - EXIT
    rm -rf "$stage"
fi

# Atomic switch: rename a fresh symlink over `current`.
ln -sfn "$hash" "$builds/.current.$$"
mv -T "$builds/.current.$$" "$builds/current"
echo "referee build $hash"
