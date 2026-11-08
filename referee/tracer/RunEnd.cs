// Referee: the end of a boss fight's rewards (XT2), compiled into the vendored sts2-cli (see referee/setup.sh).
// sts2-cli reported victory once any boss of the last act was beaten, before the game's own end: at A10 (DoubleBoss) the
// last act has a second boss, and the game only wins in TheArchitect's event room. setup.sh replaces both of sts2-cli's
// victory shortcuts (its Proceed and its post-reward path) with AfterBoss, which follows the game:
// - NRewardsScreen.OnProceedButtonPressed: a boss beaten at the boss point of a map with a second boss point only
//   returns to the map (ProceedFromTerminalRewardsScreen), whose only child of the boss point is the second boss;
// - otherwise ActChangeSynchronizer.MoveToNextAct: ActFloor++, then RunManager.EnterNextAct, which in the last act enters
//   EventRoom(TheArchitect) (RunManager.cs:1285-1310). Its PROCEED option calls RunManager.WinRun (TheArchitect.cs:350),
//   OnEnded(isVictory: true) and GuaranteeKillAllPlayers, so the next decision is `game_over`; SettledDeath reports it as
//   a victory when the room is the victory room (AbstractRoom.IsVictoryRoom, as NGameOverScreen decides).
// Before the last act sts2-cli's own EnterNextAct follows (SetActInternal resets ActFloor, so its missing ActFloor++ is
// not observable there).
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

public partial class RunSimulator
{
    // Null when sts2-cli's EnterNextAct should follow (a boss before the last act).
    private Dictionary<string, object?>? AfterBoss()
    {
        var state = _runState;
        if (state == null)
            return null;
        var map = state.Map;
        if (map.SecondBossMapPoint != null && state.CurrentMapCoord == map.BossMapPoint.coord)
        {
            Log("First of two bosses beaten, back to the map");
            ForceToMap();
            return MapSelectState();
        }
        if (state.CurrentActIndex < state.Acts.Count - 1)
            return null;
        Log("Last boss beaten, entering TheArchitect's room");
        state.ActFloor++;
        RunManager.Instance.EnterNextAct().GetAwaiter().GetResult();
        _syncCtx.Pump();
        WaitForActionExecutor();
        return DetectDecisionPoint();
    }
}
