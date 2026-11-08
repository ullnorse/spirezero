// Referee: the rest site's option task, compiled into the vendored sts2-cli (see referee/setup.sh and docs/referee-perf.md).
// sts2-cli runs a rest site option on a background task and then sleeps a fixed 200 ms, after a non-Smith option and
// after Smith's card pick, hoping the task has finished. setup.sh keeps the task here and replaces both sleeps with
// WaitForRestTask, which waits for it to finish (pumping like the other waits), so the wait is as long as the work.
//
// The combat waits (T18) replace sts2-cli's: its end_turn stopped waiting once `TurnStarted` fired, which the game also
// fires at the start of the enemy turn, and DetectDecisionPoint reported `combat_play` after a fixed 100 ms whatever the
// state. When the enemy turn yields (PunchOffEventEncounter's did), the command returned mid-turn and the next one raced
// the game thread. Now a combat command returns only once the game has settled (PlayerTurnSettled, the combat is over, or
// a prompt is open), end_turn also once the player's turn number has advanced, and a timeout is an error, not stale state.
//
// The run-mode background tasks (an event option, a shop purchase or removal, a rest site option, a run loadout) were
// waited on for a fixed number of 1 ms polls (or a fixed sleep), then the state was dumped as it stood: on a saturated
// box (PX15, the 200-seed final matrix) Self Help Book's enchant had not run when select_cards returned, so the trace
// showed the event still open. setup.sh routes those waits through WaitForGameTask / WaitForEventOption, which end when the
// task is done (or the event finished, or a prompt is open), exactly the old conditions, and report a timeout.
using System.Diagnostics;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Multiplayer;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Relics;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

public partial class RunSimulator
{
    private Task? _restTask;
    // The last event option's task (choose_option) and shop purchase's or removal's task (setup.sh).
    private Task? _eventTask;
    private Task? _shopTask;
    // The room entry of a map node whose relics buy on entering (LordsParasol's shop, PX18).
    private Task? _enterTask;
    // An event's resume after its fight (ResumeEventOnTask, RF1).
    private Task? _resumeTask;

    private void WaitForRestTask() => WaitForGameTask(_restTask);

    // LordsParasol's `PurchaseEverything` starts inside the room entry (`AfterRoomEntered`) and goes on in the continuation
    // of every prompt it opens. In the game it yields at its first UI wait (`Cmd.Wait(0.75f)`, TestMode off), so the
    // other relics' `AfterRoomEntered` (MealTicket's heal) and the rest of the entry run before the first purchase;
    // headless nothing waits, so the purchases ran inside the hook loop, on the command's thread, and a card reward
    // among them (`Orrery`'s, sts2-cli's blocking `GetSelectedCardReward`) blocked that thread for good (300 s, then every
    // reward skipped, C25) and kept the later hooks from running. The relic now sits out the entry, and its
    // `AfterRoomEntered` runs on a task once the entry is done, like a player's own purchase (`DoBuyRelic`); a prompt's
    // answer in a shop is fed to the game on a task too (ResolvePendingOffThread). setup.sh routes DoMapSelect and
    // DoSelectCards through these (PX18).
    private void EnterMapCoordOnTask(MapCoord coord)
    {
        var player = _runState!.Players[0];
        var parasol = player.Relics.OfType<LordsParasol>().FirstOrDefault();
        if (parasol == null)
        {
            RunManager.Instance.EnterMapCoord(coord).GetAwaiter().GetResult();
            return;
        }
        var relics = (List<RelicModel>)typeof(Player)
            .GetField("_relics", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance)!
            .GetValue(player)!;
        var index = relics.IndexOf(parasol);
        relics.RemoveAt(index);
        try
        {
            RunManager.Instance.EnterMapCoord(coord).GetAwaiter().GetResult();
        }
        finally
        {
            relics.Insert(index, parasol);
        }
        _syncCtx.Pump();
        if (_runState.CurrentRoom is not MerchantRoom room)
            return;
        var task = _enterTask = Task.Run(() => parasol.AfterRoomEntered(room));
        if (!PumpUntil(() => task.IsCompleted || _cardSelector.HasPendingReward))
            _settleError ??= "the room entry's purchases did not finish";
        if (task.IsFaulted)
            throw task.Exception!.GetBaseException();
    }

    private void ResolvePendingOffThread(int[] indices)
    {
        if (_runState?.CurrentRoom is not MerchantRoom)
        {
            _cardSelector.ResolvePendingByIndices(indices);
            return;
        }
        // Not HasPendingSelection: the selection being answered is still pending until the task has started.
        var task = Task.Run(() => _cardSelector.ResolvePendingByIndices(indices));
        if (!PumpUntil(() => task.IsCompleted || _cardSelector.HasPendingReward))
            _settleError ??= "the card selection's continuation did not finish";
        if (task.IsFaulted)
            throw task.Exception!.GetBaseException();
    }

    // A won event fight that resumes its event (BattlewornDummy, `ShouldResumeParentEventAfterCombat`): leaving the fight
    // runs the event's `Resume` (`ProceedFromTerminalRewardsScreen`, `ResumePreviousRoom`), which offers the room's
    // `ExtraRewards` (TheHunt's card reward) through the blocking `GetSelectedCardReward`. On the command's thread that
    // blocked until the 300 s timeout skipped it (PX27). The resume now runs on a task, like the room-entry purchases
    // above; true when it opened a card reward, which the next command answers. setup.sh routes DetectPostCombatState's
    // map exit through here (RF1).
    private bool ResumeEventOnTask(CombatRoom room)
    {
        if (!room.ShouldResumeParentEventAfterCombat || _runState!.CurrentRoomCount <= 1)
            return false;
        var task = _resumeTask = Task.Run(() => RunManager.Instance.ProceedFromTerminalRewardsScreen());
        if (!PumpUntil(() => task.IsCompleted || _cardSelector.HasPendingReward))
            _settleError ??= "the event did not resume after its fight";
        if (task.IsFaulted)
            throw task.Exception!.GetBaseException();
        return _cardSelector.HasPendingReward;
    }

    // After a shop card reward is answered: until the purchases it interrupted are done or the game prompts again.
    private void WaitForShopTasks()
    {
        if (_runState?.CurrentRoom is not MerchantRoom)
            return;
        if (!PumpUntil(() => HasPendingSelection || ((_enterTask?.IsCompleted ?? true) && (_shopTask?.IsCompleted ?? true))))
            _settleError ??= "the shop purchases did not finish";
    }

    // Pumps until `task` is done or a prompt is open (which the task waits on); no task: sts2-cli's fixed 200 ms.
    private void WaitForGameTask(Task? task)
    {
        if (task == null)
        {
            Thread.Sleep(200);
            return;
        }
        if (!PumpUntil(() => task.IsCompleted || HasPendingSelection))
            _settleError ??= "a background game task did not finish";
        ReportFault(task);
    }

    // A game task that threw left the game half-way through what it was doing; the dump would show that as if it
    // were the game's own state (RF2), so the command fails instead.
    private void ReportFault(Task? task)
    {
        if (task is { IsFaulted: true })
            _settleError ??= $"a background game task failed: {task.Exception!.GetBaseException()}";
    }

    // After a card pick or a card reward in an event: until the option's task is done, the event is finished (or gone)
    // or the game prompts again.
    private void WaitForEventOption()
    {
        if (!PumpUntil(() => HasPendingSelection || _eventTask is { IsCompleted: true }
                || RunManager.Instance.EventSynchronizer?.GetLocalEvent() is not { IsFinished: false }))
            _settleError ??= "the event option did not finish";
        ReportFault(_eventTask);
    }

    // After a pick or skip of the game's blocking card choice (referee/setup.sh). In a rest site the choice is a heal's
    // reward (DreamCatcher); once the option task is done the rest site is left like after any finished option. After an
    // event fight's resume (ResumeEventOnTask) the resume finishes first, or prompts again.
    private Dictionary<string, object?> AfterRewardAnswered()
    {
        if (_resumeTask is { } resume)
        {
            if (!PumpUntil(() => HasPendingSelection || resume.IsCompleted))
                _settleError ??= "the event did not resume after its fight";
            if (resume.IsFaulted)
                throw resume.Exception!.GetBaseException();
            if (resume.IsCompleted)
                _resumeTask = null;
            WaitForActionExecutor();
        }
        if (_runState?.CurrentRoom is RestSiteRoom && !_cardSelector.HasPending && !_cardSelector.HasPendingReward)
        {
            WaitForRestTask();
            _syncCtx.Pump();
            WaitForActionExecutor();
            ForceToMap();
            return MapSelectState();
        }
        return DetectDecisionPoint();
    }

    // A won fight whose encounter gives no rewards (`ShouldGiveRewards` false, the BattlewornDummy fights) skips sts2-cli's
    // reward generation (referee/setup.sh), whose pumping is what let the combat's end run to completion before the dump:
    // `CombatOver` holds as soon as `EndCombatInternal` clears `IsInProgress`, before `AfterCombatEnd` (VelvetChoker's
    // reset) has run. Wait for the room to be marked finished, after `AfterCombatEnd` and `AfterCombatVictory` (C24).
    private void WaitForCombatEnd(CombatRoom room)
    {
        if (!PumpUntil(() => room.IsPreFinished))
            _settleError ??= "the combat did not finish ending";
    }

    // How long a command may wait for the game to settle before it reports an error.
    private const int SettleTimeoutMs = 30_000;

    // Set by a wait that timed out during the current action; ExecuteAction turns it into an error.
    private string? _settleError;

    public Dictionary<string, object?> ExecuteAction(string action, Dictionary<string, object?>? args)
    {
        _settleError = null;
        var result = ExecuteActionUpstream(action, args);
        return _settleError is { } error ? Error($"{action}: {error}") : result;
    }

    // Pumps the game's continuations until `done` holds; false after SettleTimeoutMs.
    private bool PumpUntil(Func<bool> done)
    {
        var clock = Stopwatch.StartNew();
        while (true)
        {
            _syncCtx.Pump();
            if (done())
                return true;
            if (clock.ElapsedMilliseconds > SettleTimeoutMs)
                return false;
            Thread.Sleep(1);
        }
    }

    private void WaitForActionExecutor()
    {
        try
        {
            SynchronizationContext.SetSynchronizationContext(_syncCtx);
            _syncCtx.Pump();
            // The executor stays running while an action waits on the headless card selection (BUG-026).
            if (!PumpUntil(() => HasPendingSelection || !RunManager.Instance.ActionExecutor.IsRunning))
                _settleError ??= "the action executor did not finish";
        }
        catch (Exception ex)
        {
            Log($"WaitForActionExecutor exception: {ex.Message}");
        }
    }

    // The player can act: their turn's play phase has begun (the game sets PlayPhase and unpauses the executor right
    // before TurnStarted, CombatManager.StartTurn) and every queued action has finished. A turn the player has already
    // ended (a play's PlayerCmd.EndTurn, Void Form) is not settled: its end is a continuation that runs after the play's
    // action finishes, so the check would pass on the old turn; StartTurn clears the flag before the next play phase.
    private bool PlayerTurnSettled()
    {
        var combat = CombatManager.Instance;
        return combat.IsInProgress
            && combat.DebugOnlyGetState()?.CurrentSide == CombatSide.Player
            && IsPlayPhase()
            && RunManager.Instance.ActionQueueSynchronizer.CombatState == ActionSynchronizerCombatState.PlayPhase
            && !combat.IsPlayerReadyToEndTurn(_runState!.Players[0])
            && !RunManager.Instance.ActionExecutor.IsRunning
            && RunManager.Instance.ActionQueueSet.IsEmpty;
    }

    // The combat has ended and its last action finished. A death only marks the loss; the game ends the combat at its
    // next safe point (CombatManager.LoseCombat), so a dead player alone is not settled. A combat that has not started
    // yet is not over either: `IsInProgress` is false from `SetUpCombat` until `StartCombatInternal` runs on the turn
    // loop (`IsStarting` until then), and the turn loop first waits for the previous combat's to die
    // (`WaitForPreviousTurnLoopToFinish`). After an event fight (BattlewornDummy) that wait outlived select_map_node's
    // settle on a loaded box, so the next fight counted as won before it began and its rewards were rolled (PX23).
    private static bool CombatOver() => !CombatManager.Instance.IsInProgress && !CombatManager.Instance.IsStarting
        && !RunManager.Instance.ActionExecutor.IsRunning;

    private string CombatWaitState(Player player)
    {
        var state = CombatManager.Instance.DebugOnlyGetState();
        return $"round {state?.RoundNumber}, side {state?.CurrentSide}, turn {player.PlayerCombatState?.TurnNumber}, "
            + $"phase {player.PlayerCombatState?.Phase}, sync {RunManager.Instance.ActionQueueSynchronizer.CombatState}, "
            + $"executor running {RunManager.Instance.ActionExecutor.IsRunning}, queue empty {RunManager.Instance.ActionQueueSet.IsEmpty}";
    }

    // DetectDecisionPoint's game over on death: once the combat the death happened in has ended.
    private Dictionary<string, object?> SettledDeath()
    {
        if (!PumpUntil(CombatOver))
            return Error($"the combat did not end after the player died ({CombatWaitState(_runState!.Players[0])})");
        // TheArchitect's WinRun kills the player after ending the run as a victory (referee/tracer/RunEnd.cs).
        return GameOverState(_runState!.CurrentRoom?.IsVictoryRoom ?? false);
    }

    // A turn-start prompt's answer (Entropy's transform) can kill the last enemy (Smokestack's damage) while the start of
    // the turn is still waiting on it. In the game the prompt is a hook GameAction: `StartTurn` goes on to the play phase,
    // the action runs there, and `ActionExecutor` calls `CheckWinCondition` after it, which ends the won fight
    // (`CardSelectCmd.FromHand` signals the player choice only without a test selector, which sts2-cli sets, so here the
    // hook resumes inline inside `SetupPlayerTurn` while the executor is paused and nothing checks the win again: the
    // turn start never completes, PX21). Once the combat has sat in that state, the check the executor would make is made.
    private Task? _wonTurnStartCheck;
    private Stopwatch? _wonTurnStartClock;

    private bool WonInsideTurnStart(Player player)
    {
        var combat = CombatManager.Instance;
        var state = combat.DebugOnlyGetState();
        var stuck = combat.IsInProgress
            && state?.CurrentSide == CombatSide.Player
            && player.PlayerCombatState?.Phase == PlayerTurnPhase.Start
            && !state.Enemies.Any(e => e.IsAlive && e.IsPrimaryEnemy)
            && !RunManager.Instance.ActionExecutor.IsRunning
            && RunManager.Instance.ActionQueueSet.IsEmpty
            && !HasPendingSelection;
        if (!stuck)
        {
            _wonTurnStartClock = null;
            _wonTurnStartCheck = null;
            return false;
        }
        _wonTurnStartClock ??= Stopwatch.StartNew();
        if (_wonTurnStartCheck == null && _wonTurnStartClock.ElapsedMilliseconds > 250)
            _wonTurnStartCheck = combat.CheckWinCondition();
        return _wonTurnStartCheck is { IsFaulted: true } faulted ? throw faulted.Exception!.GetBaseException() : false;
    }

    // DetectDecisionPoint's combat room, after its prompt check: the settled decision, or null when a prompt opened.
    private Dictionary<string, object?>? SettledCombatDecision(Player player, CombatRoom room)
    {
        _wonTurnStartClock = null;
        _wonTurnStartCheck = null;
        if (!PumpUntil(() => HasSurfaceablePrompt || CombatOver() || PlayerTurnSettled() || WonInsideTurnStart(player)))
            return Error($"combat did not settle ({CombatWaitState(player)})");
        if (HasSurfaceablePrompt)
            return null;
        return CombatOver() ? DetectPostCombatState(player, room) : CombatPlayState(player);
    }

    // Replaces sts2-cli's DoEndTurn (kept as DoEndTurnUpstream), whose cancel-and-retry fallbacks changed the game.
    private Dictionary<string, object?> DoEndTurn(Player player)
    {
        // A pending prompt must be answered first (#61).
        if (HasPendingSelection)
            return DetectDecisionPoint();
        if (!PumpUntil(() => HasPendingSelection || CombatOver() || PlayerTurnSettled()))
            return Error($"end_turn: the turn did not settle before ending it ({CombatWaitState(player)})");
        if (HasPendingSelection || CombatOver())
            return DetectDecisionPoint();

        var turn = player.PlayerCombatState!.TurnNumber;
        Log($"Ending turn (round={CombatManager.Instance.DebugOnlyGetState()?.RoundNumber ?? 0})");
        bool settled;
        // As sts2-cli does: Task.Yield runs inline while the enemy turn plays (boss fights deadlock otherwise).
        YieldPatches.SuppressYield = true;
        try
        {
            PlayerCmd.EndTurn(player, canBackOut: false);
            // An enemy-turn prompt (KnowledgeDemon) is handed to the caller; select_cards resumes the turn.
            settled = PumpUntil(() => HasPendingSelection || CombatOver()
                || (player.PlayerCombatState.TurnNumber > turn && PlayerTurnSettled()));
        }
        finally
        {
            YieldPatches.SuppressYield = false;
        }
        if (!settled)
            return Error($"end_turn: the next turn did not start ({CombatWaitState(player)})");
        return DetectDecisionPoint();
    }
}
