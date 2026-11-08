// Referee: answer a run's reward sets the way the game's own TestMode path expects.
//
// `RewardsSet.Offer` in TestMode takes every reward, then throws "The RewardsSet is not complete after rewards were
// selected!" when one could not be taken (a PotionReward with a full belt: `PotionReward.OnSelect` returns false on
// `TooFull`). The game's own tests install `RewardsSet.testSelector` for this; sts2-cli never did, so before RF2 the
// exception silently faulted the event's, rest site's or shop's task and the run continued on a partly applied state.
// RF2's fault check made it an error and 56 run loadouts failed (PotionCourier, Wellspring, the rest-site heal with
// TinyMailbox/DreamCatcher).
//
// The selector takes what it can and then skips the rest, which is what a player does on the rewards screen and what
// `SkipLocalRewardsSet` records (the unselected reward's `OnSkipped`, the set completed as skipped). Nothing is decided
// here that the engine does not also see: a full belt takes nothing, an open slot takes the potion.
using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Runs;

internal static class RefereeRewards
{
    [ModuleInitializer]
    internal static void Install() => RewardsSet.testSelector = SelectAll;

    private static async Task SelectAll(RewardsSet set)
    {
        foreach (Reward reward in new List<Reward>(set.Rewards))
        {
            await reward.SelectUnsynchronized();
        }
        if (!RunManager.Instance.RewardsSetSynchronizer.IsRewardsSetCompleted(set))
        {
            RunManager.Instance.RewardsSetSynchronizer.SkipLocalRewardsSet();
        }
    }
}
