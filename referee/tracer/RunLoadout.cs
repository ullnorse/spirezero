// `run_loadout` command compiled into the vendored sts2-cli (see referee/setup.sh): the run-mode
// counterpart of `set_loadout`. Adds to a run that waits on the map (after Neow) instead of
// replacing the starters: `{"act_index": N, "remove_relics": [...], "remove_cards": [...], "add_cards": [...], "gold": N,
// "add_relics": [...], "max_hp": N, "hp": N}`, applied in that order. `remove_relics` takes each id's first relic off
// (`RemoveRelicInternal`, no hooks) and `remove_cards` every deck card of each id (`RemoveInternal`), so an ancient can
// be entered without a starter (Orobas's locked option). `act_index` enters that act first (`RunManager.EnterAct` without the
// transition, as sts2-cli's start_run enters act 0), so an act-gated event can be forced in its act. Card ids take the `Loadout` syntax (`ID[+N][:ENCHANTMENT[*A]]`) and join the
// deck with `AddInternal` like `set_loadout`'s. Relics go through `RelicCmd.Obtain` on a task, like an
// event option's relic, so `AfterObtained` runs and a prompt it opens (a card selection, a rewards
// screen) is the answer, as after any action; only the last relic may prompt.
using System.Text.Json;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

public partial class RunSimulator
{
    // The task of the relic whose prompt is pending; a later select_cards / select_card_reward finishes it.
    private Task? _loadoutTask;

    // Waits for the relic's task to finish or prompt again. TestMode's `RewardsSet.Offer` throws when the set
    // cannot be taken whole (Cauldron's 5 potions on 3 slots); the relic has joined by then, so that is not an error.
    private void WaitForLoadoutTask()
    {
        var task = _loadoutTask;
        if (task == null) return;
        if (!PumpUntil(() => task.IsCompleted || HasPendingSelection))
            _settleError ??= "the run loadout did not finish";
        _syncCtx.Pump();
        if (task.IsCompleted)
        {
            _loadoutTask = null;
            if (task.IsFaulted && !(task.Exception?.GetBaseException().Message.Contains("RewardsSet is not complete") ?? false))
                throw task.Exception!.GetBaseException();
        }
        WaitForActionExecutor();
    }

    public Dictionary<string, object?> ApplyRunLoadout(JsonElement cmd)
    {
        try
        {
            var run = RunManager.Instance.DebugOnlyGetState();
            if (run == null) return Loadout.Error("No run in progress");
            if (run.CurrentRoom is not MegaCrit.Sts2.Core.Rooms.MapRoom)
                return Loadout.Error("run_loadout needs the map (after Neow)");
            var player = run.Players[0];

            if (cmd.TryGetProperty("act_index", out var actIndex))
            {
                int index = actIndex.GetInt32();
                if (index <= run.CurrentActIndex || index >= run.Acts.Count)
                    return Loadout.Error($"act_index {index} is not a later act");
                RunManager.Instance.EnterAct(index, doTransition: false).GetAwaiter().GetResult();
                WaitForActionExecutor();
            }

            if (cmd.TryGetProperty("remove_relics", out var removeRelics))
            {
                foreach (var id in removeRelics.EnumerateArray().Select(e => e.GetString()))
                {
                    var relic = player.Relics.FirstOrDefault(r => r.Id.Entry == id);
                    if (relic == null) return Loadout.Error($"remove_relics: no relic {id}");
                    player.RemoveRelicInternal(relic);
                }
            }
            if (cmd.TryGetProperty("remove_cards", out var removeCards))
            {
                foreach (var id in removeCards.EnumerateArray().Select(e => e.GetString()))
                {
                    var matching = player.Deck.Cards.Where(c => c.Id.Entry == id).ToList();
                    if (matching.Count == 0) return Loadout.Error($"remove_cards: no card {id}");
                    foreach (var card in matching) player.Deck.RemoveInternal(card);
                }
            }

            var cards = Loadout.Resolve<CardModel>(cmd, "add_cards", "CARD", out var upgrades, out var enchants, out var counters);
            var relics = Loadout.Resolve<RelicModel>(cmd, "add_relics", "RELIC", out _, out _, out _);

            if (cards != null)
            {
                for (int i = 0; i < cards.Count; i++)
                {
                    var card = run.CreateCard(cards[i], player);
                    for (int u = 0; u < upgrades[i] && card.IsUpgradable; u++)
                    {
                        card.UpgradeInternal();
                        card.FinalizeUpgradeInternal();
                    }
                    if (enchants[i] is var (enchantment, amount))
                        CardCmd.Enchant(enchantment.ToMutable(), card, amount);
                    Loadout.SetCounter(card, counters[i]);
                    player.Deck.AddInternal(card);
                }
            }

            if (cmd.TryGetProperty("gold", out var gold)) player.Gold = gold.GetInt32();

            if (relics != null)
            {
                for (int i = 0; i < relics.Count; i++)
                {
                    var relic = relics[i];
                    var task = _loadoutTask = Task.Run(() => RelicCmd.Obtain(relic.ToMutable(), player).GetAwaiter().GetResult());
                    for (int t = 0; t < 2000; t++)
                    {
                        _syncCtx.Pump();
                        if (HasPendingSelection) break;
                        if (task.IsCompleted) break;
                        Thread.Sleep(1);
                    }
                    if (HasPendingSelection)
                    {
                        if (i + 1 < relics.Count)
                            return Loadout.Error($"relic {relic.Id.Entry} prompts, so it must be the last one");
                        if (cmd.TryGetProperty("hp", out _) || cmd.TryGetProperty("max_hp", out _))
                            return Loadout.Error("hp and max_hp cannot follow a relic that prompts");
                        WaitForActionExecutor();
                        return DetectDecisionPoint();
                    }
                    WaitForLoadoutTask();
                }
            }

            // Last, so explicit values win over anything a relic's AfterObtained did to max HP.
            if (cmd.TryGetProperty("max_hp", out var mhp)) player.Creature.SetMaxHpInternal(mhp.GetInt32());
            if (cmd.TryGetProperty("hp", out var hp)) player.Creature.SetCurrentHpInternal(hp.GetInt32());

            WaitForActionExecutor();
            return DetectDecisionPoint();
        }
        catch (Exception ex)
        {
            return Loadout.Error($"{ex.GetType().Name}: {ex.Message}");
        }
    }
}
