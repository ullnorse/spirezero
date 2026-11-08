// Referee: `card_alternative`, a card reward's non-card option (Driftwood's REROLL, PaelsWing's SACRIFICE), compiled into
// the vendored sts2-cli (see referee/setup.sh, PX21).
//
// The game offers them on the card reward screen (`CardRewardAlternative.Generate`: Skip, REROLL while `CanReroll`, then
// the relics' ones, in that order) and `CardReward.OnSelect` runs the chosen one's callback, after which the screen stays
// (`DoNothing`, the reroll), the reward is complete (`EndSelectionAndCompleteReward`, the sacrifice) or the screen closes
// with the reward left (`EndSelectionAndDoNotCompleteReward`, Skip). sts2-cli never goes through `OnSelect` for a fight's
// card reward (it adds the picked card itself) and its test selector had no way to answer with an alternative, so the
// protocol had no such action. `args.alternative` is the index into the game's alternatives list, Skip first.
//
//   - A fight's card reward (`_pendingCardReward`): the alternative's callback is run here, then like sts2-cli's pick or
//     skip: a completed reward is closed (`OnSkipped` is `OnSelect`'s tail after the loop: the history entries of the
//     cards not taken and the unsubscription from `Player.RelicObtained`, which a relic the callback obtained still
//     triggers first), a reroll shows the new cards.
//   - An event's, Neow's or a shop relic's card reward is the game's own `OnSelect` loop asking the test selector
//     (`GetSelectedCardReward`); setup.sh lets the selector answer with an alternative (ResolveRewardAlternative), and the
//     loop does the rest.
using MegaCrit.Sts2.Core.Entities.CardRewardAlternatives;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rewards;

namespace Sts2Headless;

public partial class RunSimulator
{
    private Dictionary<string, object?> DoCardAlternative(Player player, Dictionary<string, object?>? args)
    {
        if (args == null || !args.ContainsKey("alternative"))
            return Error("card_alternative requires 'alternative'");
        var index = Convert.ToInt32(args["alternative"]);

        if (_cardSelector.HasPendingReward)
        {
            var count = _cardSelector.PendingRewardAlternatives?.Count ?? 0;
            if (index < 0 || index >= count)
                return Error($"Invalid card reward alternative {index}, {count} available");
            Log($"Resolving event card reward: alternative {index}");
            var answered = _cardSelector.PendingRewardCards;
            _cardSelector.ResolveRewardAlternative(index);
            WaitForEventTask(answered);
            Thread.Sleep(50);
            _syncCtx.Pump();
            WaitForActionExecutor();
            return AfterRewardAnswered();
        }

        if (_pendingCardReward is not { } reward)
            return Error("No pending card reward");
        var alternatives = CardRewardAlternative.Generate(reward);
        if (index < 0 || index >= alternatives.Count)
            return Error($"Invalid card reward alternative {index}, {alternatives.Count} available");
        var alternative = alternatives[index];
        Log($"Selected card reward alternative: {alternative.OptionId}");
        if (alternative.AfterSelected == PostAlternateCardRewardAction.EndSelectionAndDoNotCompleteReward)
            return DoSkipCardReward(player);

        var task = alternative.OnSelect();
        if (!PumpUntil(() => task.IsCompleted || HasPendingSelection))
            _settleError ??= "the card reward alternative did not finish";
        if (task.IsFaulted)
            throw task.Exception!.GetBaseException();
        if (alternative.AfterSelected == PostAlternateCardRewardAction.EndSelectionAndCompleteReward)
        {
            _pendingCardReward = null;
            // A prompt the callback opened (a relic's) keeps the task: the reward closes when it is answered.
            if (task.IsCompleted)
                reward.OnSkipped();
            else
                task.ContinueWith(_ => reward.OnSkipped(), TaskContinuationOptions.ExecuteSynchronously);
        }
        _syncCtx.Pump();
        WaitForActionExecutor();
        return DetectDecisionPoint();
    }
}
