// Referee: sts2-cli's runtime Harmony patches are turned off (referee/setup.sh replaces every `new Harmony(..)` in
// RunSimulator.cs with RefereeNoHarmony.Create).
//
// The project rolls forward to the newest installed .NET (`RollForward=LatestMajor`). On .NET 10 Harmony cannot patch
// (`CoreCLR version 10.0.13 is not supported`), so every patch site failed into its own catch and the referee ran the
// game's own code: TestMode's bundle 0 in FromChooseABundleScreen, RewardsSet.Offer's TestMode path, LocTable throwing
// on a missing key. With only .NET 9 installed the same build applied them, and the traces changed (relic titles as
// keys, a bundle prompt after Neow, event reward sets bypassed; PX16). Throwing where Harmony would be created keeps
// the referee the game, whatever runtime is installed; the IL patches on sts2.dll (nullguard, patcher) still apply.
using System;
using HarmonyLib;

internal static class RefereeNoHarmony
{
    public static Harmony Create(string id) =>
        throw new NotSupportedException($"referee: Harmony patch {id} is disabled (referee/tracer/NoHarmony.cs)");
}
