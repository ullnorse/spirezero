// Referee: `reset`, so that one process serves many seeds (referee/README.md, "One process, many seeds").
//
// Ends the current run the way the game does between runs (RunManager.CleanUp), replaces the RunSimulator (all its
// per-run fields), and clears the process-wide state that outlives a run and is not cleared by CleanUp in test mode:
//   - a choice the last seed left open (its actions ended on a card_select, bundle_select or event card reward).
//     The game code that asked is parked on the old selector's TaskCompletionSource, and with it the combat's turn
//     loop when a hook asked (EntropyPower.AfterPlayerTurnStart). CombatManager keeps that loop across Reset
//     (`_turnLoopTask`) and the next combat's loop waits up to 10 s for it to die before it starts, so the next fight
//     is not in progress when sts2-cli looks and reads as won ("could not reach combat", T14). The choice is cancelled
//     after CleanUp so the parked code dies of cancellation (the game's teardown path); an event card reward blocks a
//     pool thread instead, so it is skipped and drained before CleanUp. Reset then waits for the old turn loop to
//     finish: one still dying on a pool thread makes the next combat's start asynchronous, and sts2-cli's EnterRoom
//     reads it as over under load. A loop still alive after 5 s is reported (`stale_turn_loop`) and referee.py
//     replaces the process;
//   - CardSelectCmd's selector stacks (CardSelectCmd.Reset only clears them with TestMode off; StartRun's
//     UseSelector throws if one is left);
//   - sts2-cli's own statics (killing-blow bookkeeping, the Task.Yield suppression flag);
//   - the environment variables the run reads (STS2_ACT1 at StartRun, STS2_REAL_* in the patched TestMode sites),
//     which the command sets for the next run.
// Everything else the run depends on is per-run (RunState, its RNG sets, the players' UnlockState.all) or immutable
// after start-up (ModelDb's canonical models, the localisation tables). Known process-wide leftovers, none observable:
// CardRarityOdds.regularCommonOdds (fixed by the first run's ascension, never read by a roll), CombatTurnState's
// CombatId counter (only compared for equality), YummyCookie's canonical icon character, Rng.Chaotic (time-seeded,
// canonical models only), and the old simulators' CombatManager event handlers (they only set their own events).

using System.Reflection;
using System.Text.Json;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

public partial class RunSimulator
{
    // An event card reward blocks a pool thread in GetSelectedCardReward: skip it like skip_card_reward does and wait
    // for the event task, while the run still exists. Bounded, an event may offer another reward.
    internal void DrainPendingReward()
    {
        for (int i = 0; i < 5 && _cardSelector.HasPendingReward; i++)
        {
            RunManager.Instance.RewardsSetSynchronizer.SkipLocalRewardsSet();
            var answered = _cardSelector.PendingRewardCards;
            _cardSelector.SkipReward();
            WaitForEventTask(answered);
        }
    }

    // Pumps until `task` is done (at most 5 s); false if it is still running.
    internal bool WaitForTask(Task? task)
    {
        for (int i = 0; i < 5000 && task is { IsCompleted: false }; i++)
        {
            _syncCtx.Pump();
            Thread.Sleep(1);
        }
        return task is not { IsCompleted: false };
    }

    // Cancels the open card or bundle choice, so the code awaiting it throws OperationCanceledException.
    internal void CancelPendingChoices()
    {
        var tcs = (TaskCompletionSource<IEnumerable<CardModel>>?)typeof(HeadlessCardSelector)
            .GetField("_pendingTcs", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(_cardSelector);
        foreach (var pending in new[] { tcs, _pendingBundleTcs })
        {
            SynchronizationContext.SetSynchronizationContext(_syncCtx);
            pending?.TrySetCanceled();
            _syncCtx.Pump();
        }
    }
}

public static class RefereeReset
{
    private static readonly string[] EnvVars = { "STS2_ACT1", "STS2_REAL_SHOP", "STS2_REAL_CAULDRON", "STS2_REAL_BELL" };

    private static readonly FieldInfo TurnLoopTask =
        typeof(CombatManager).GetField("_turnLoopTask", BindingFlags.NonPublic | BindingFlags.Instance)!;

    public static Dictionary<string, object?> Reset(ref RunSimulator sim, JsonElement cmd)
    {
        sim.DrainPendingReward();
        sim.CleanUp();
        // sim.CleanUp skips a run that is over (IsInProgress false) but still set, which StartRun rejects.
        RunManager.Instance.CleanUp(graceful: true);
        sim.CancelPendingChoices();
        bool staleTurnLoop = !sim.WaitForTask(TurnLoopTask.GetValue(CombatManager.Instance) as Task);
        sim = new RunSimulator();

        foreach (var name in new[] { "_selectorStack", "_localSelectorStack" })
        {
            var stack = typeof(CardSelectCmd).GetField(name, BindingFlags.NonPublic | BindingFlags.Static)!.GetValue(null)!;
            stack.GetType().GetMethod("Clear")!.Invoke(stack, null);
        }
        RunSimulator.LastKnownPlayerHp = 80;
        RunSimulator.LastActionWasKillingBlow = false;
        RunSimulator.YieldPatches.SuppressYield = false;

        var env = cmd.TryGetProperty("env", out var e) ? e : default;
        foreach (var name in EnvVars)
        {
            string? value = env.ValueKind == JsonValueKind.Object && env.TryGetProperty(name, out var v) ? v.GetString() : null;
            Environment.SetEnvironmentVariable(name, value);
        }
        return new Dictionary<string, object?> { ["type"] = "reset", ["stale_turn_loop"] = staleTurnLoop };
    }
}
