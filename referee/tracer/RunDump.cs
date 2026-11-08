// Read-only run-level state dumper compiled into the vendored sts2-cli (see referee/setup.sh).
// `dump_run` reads the live game objects through their public API (plus reflection for sts2-cli's
// own pending-choice fields); it never mutates game state and never advances an RNG.
using System.Reflection;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

public static class RunDump
{
    private const BindingFlags Private = BindingFlags.Instance | BindingFlags.NonPublic;

    /// <param name="sim">sts2-cli's simulator, only read for its pending-choice fields.</param>
    /// <param name="withMap">Include the full map and room lists (once per act).</param>
    /// <param name="withActMaps">Also include `act_maps`: every act's standard map, built like
    /// `StandardActMap.CreateFor` would for that act (no map hooks), so later acts can be diffed at step 0.</param>
    public static Dictionary<string, object?> Dump(RunSimulator sim, bool withMap, bool withActMaps = false)
    {
        var run = RunManager.Instance.DebugOnlyGetState();
        if (run == null) return new() { ["type"] = "error", ["message"] = "No run in progress" };
        var player = run.Players[0];

        var result = new Dictionary<string, object?>
        {
            ["type"] = "run",
            ["act"] = run.CurrentActIndex + 1,
            ["act_id"] = run.Act.Id.Entry,
            ["floor"] = run.ActFloor,
            ["total_floor"] = run.TotalFloor,
            ["coord"] = run.CurrentMapCoord is { } c ? new[] { c.col, c.row } : null,
            ["visited"] = run.VisitedMapCoords.Select(v => new[] { v.col, v.row }).ToList(),
            ["room"] = run.CurrentRoom?.GetType().Name,
            ["room_type"] = run.CurrentRoom?.RoomType.ToString(),
            ["hp"] = player.Creature.CurrentHp,
            ["max_hp"] = player.Creature.MaxHp,
            ["gold"] = player.Gold,
            ["deck"] = player.Deck.Cards.Select(Card).ToList(),
            ["relics"] = player.Relics.Select(r => r.Id.Entry).ToList(),
            ["relic_state"] = player.Relics.Select(TraceDump.Relic).ToList(),
            ["removals_used"] = player.ExtraFields.CardShopRemovalsUsed,
            ["potions"] = player.PotionSlots.Select(p => p?.Id.Entry).ToList(),
            ["rng"] = Rngs(run, player),
            ["odds"] = Odds(run, player),
            ["rooms"] = Rooms(run, withMap),
            ["events_seen"] = run.VisitedEventIds.Select(e => e.Entry).Order().ToList(),
            ["pending"] = Pending(sim, run),
        };
        if (withMap)
        {
            result["map"] = Map(run);
            result["unlock_number_of_runs"] = run.UnlockState.NumberOfRuns;
        }
        if (withActMaps)
            result["act_maps"] = run.Acts.Select((act, i) => MapJson(new StandardActMap(
                new Rng(run.Rng.Seed, $"act_{i + 1}_map"), act, run.Players.Count > 1, false, act.HasSecondBoss))).ToList();
        return result;
    }

    // `vars` as in a combat trace (TraceDump.Vars): the deck card's own `DynamicVars` (C23).
    // `enchantment_amount` and `fields` (ModelFields, only when the card class declares any) since XT3.
    private static Dictionary<string, object?> Card(CardModel c)
    {
        var card = new Dictionary<string, object?>
        {
            ["id"] = c.Id.Entry,
            ["upgrade"] = c.CurrentUpgradeLevel,
            ["enchantment"] = c.Enchantment?.Id.Entry,
            ["enchantment_amount"] = c.Enchantment?.Amount,
            ["vars"] = TraceDump.Vars(c),
        };
        var fields = ModelFields.Of(c, typeof(CardModel), null);
        if (fields.Count > 0) card["fields"] = fields;
        return card;
    }

    // Call counts of every run-scoped and player-scoped stream (`Rng._counter`, via ToSerializable).
    internal static Dictionary<string, object?> Rngs(RunState run, Player player)
    {
        var run_ = new Dictionary<string, object?>();
        foreach (var t in Enum.GetValues<RunRngType>())
            run_[t.ToString()] = run.Rng.GetRng(t).ToSerializable().counter;
        var player_ = new Dictionary<string, object?>();
        foreach (var t in Enum.GetValues<PlayerRngType>())
            player_[t.ToString()] = player.PlayerRng.GetRng(t).ToSerializable().counter;
        return new() { ["run"] = run_, ["player"] = player_ };
    }

    // f32 values; System.Text.Json prints the shortest string that round-trips a float.
    private static Dictionary<string, object?> Odds(RunState run, Player player)
    {
        var u = run.Odds.UnknownMapPoint;
        return new()
        {
            ["card_rarity_offset"] = player.PlayerOdds.CardRarity.CurrentValue,
            ["potion_reward"] = player.PlayerOdds.PotionReward.CurrentValue,
            ["unknown_monster"] = u.MonsterOdds,
            ["unknown_elite"] = u.EliteOdds,
            ["unknown_treasure"] = u.TreasureOdds,
            ["unknown_shop"] = u.ShopOdds,
        };
    }

    // Visit counters every time; the pre-rolled lists only with the map (they never change within an act).
    private static Dictionary<string, object?> Rooms(RunState run, bool withLists)
    {
        var set = typeof(ActModel).GetField("_rooms", Private)?.GetValue(run.Act) as RoomSet;
        if (set == null) return new();
        var r = new Dictionary<string, object?>
        {
            ["events_visited"] = set.eventsVisited,
            ["normal_visited"] = set.normalEncountersVisited,
            ["elite_visited"] = set.eliteEncountersVisited,
            ["boss_visited"] = set.bossEncountersVisited,
        };
        if (withLists)
        {
            r["events"] = set.events.Select(e => e.Id.Entry).ToList();
            r["normals"] = set.normalEncounters.Select(e => e.Id.Entry).ToList();
            r["elites"] = set.eliteEncounters.Select(e => e.Id.Entry).ToList();
            r["boss"] = set.Boss.Id.Entry;
            r["second_boss"] = set.SecondBoss?.Id.Entry;
            r["ancient"] = set.HasAncient ? set.Ancient.Id.Entry : null;
        }
        return r;
    }

    // Every node with its type and children (children sorted by coord, so the dump does not depend on HashSet order).
    private static Dictionary<string, object?> Map(RunState run) => MapJson(run.Map);

    private static Dictionary<string, object?> MapJson(ActMap map)
    {
        object Node(MapPoint p) => new Dictionary<string, object?>
        {
            ["col"] = p.coord.col,
            ["row"] = p.coord.row,
            ["type"] = p.PointType.ToString(),
            ["children"] = p.Children.OrderBy(ch => ch.coord.row).ThenBy(ch => ch.coord.col)
                .Select(ch => new[] { ch.coord.col, ch.coord.row }).ToList(),
        };
        return new()
        {
            ["nodes"] = map.GetAllMapPoints().OrderBy(p => p.coord.row).ThenBy(p => p.coord.col).Select(Node).ToList(),
            ["start"] = Node(map.StartingMapPoint),
            ["boss"] = Node(map.BossMapPoint),
            ["second_boss"] = map.SecondBossMapPoint is { } b ? Node(b) : null,
        };
    }

    // The choice the run is waiting on, read from the game's own objects.
    private static Dictionary<string, object?>? Pending(RunSimulator sim, RunState run)
    {
        // sts2-cli prompts first (see DetectDecisionPoint): card select, then card reward.
        var selector = typeof(RunSimulator).GetField("_cardSelector", Private)?.GetValue(sim);
        if (selector != null && Prop<bool>(selector, "HasPending") && Prop<List<CardModel>?>(selector, "PendingOptions") is { } opts)
            return new()
            {
                ["kind"] = "card_select",
                ["cards"] = opts.Select(Card).ToList(),
                ["min"] = Prop<int>(selector, "PendingMinSelect"),
                ["max"] = Prop<int>(selector, "PendingMaxSelect"),
            };
        if (selector != null && Prop<bool>(selector, "HasPendingReward") && Prop<List<CardCreationResult>?>(selector, "PendingRewardCards") is { } eventCards)
            return new() { ["kind"] = "event_card_reward", ["cards"] = eventCards.Select(cr => Card(cr.Card)).ToList() };
        if (typeof(RunSimulator).GetField("_pendingCardReward", Private)?.GetValue(sim) is CardReward reward)
            return new() { ["kind"] = "card_reward", ["cards"] = reward.Cards.Select(Card).ToList(), ["can_skip"] = reward.CanSkip };

        switch (run.CurrentRoom)
        {
            case EventRoom:
                var ev = RunManager.Instance.EventSynchronizer?.GetLocalEvent();
                if (ev == null) return null;
                return new()
                {
                    ["kind"] = "event",
                    ["event"] = ev.Id.Entry,
                    ["finished"] = ev.IsFinished,
                    ["options"] = ev.CurrentOptions.Select(o => new Dictionary<string, object?>
                    {
                        ["text_key"] = o.TextKey,
                        ["locked"] = o.IsLocked,
                        ["chosen"] = o.WasChosen,
                    }).ToList(),
                };
            case RestSiteRoom rest:
                return new()
                {
                    ["kind"] = "rest",
                    ["options"] = (rest.Options ?? new List<MegaCrit.Sts2.Core.Entities.RestSite.RestSiteOption>()).Select(o => new Dictionary<string, object?>
                    {
                        ["id"] = o.OptionId,
                        ["enabled"] = o.IsEnabled,
                    }).ToList(),
                };
            case MerchantRoom shop:
                var inv = shop.GetLocalInventory();
                if (inv == null) return null;
                return new()
                {
                    ["kind"] = "shop",
                    ["cards"] = inv.CharacterCardEntries.Concat(inv.ColorlessCardEntries).Select(e => new Dictionary<string, object?>
                    {
                        ["card"] = e.CreationResult?.Card is { } c ? Card(c) : null,
                        ["cost"] = e.Cost,
                        ["on_sale"] = e.IsOnSale,
                        ["stocked"] = e.IsStocked,
                    }).ToList(),
                    ["relics"] = inv.RelicEntries.Select(e => new Dictionary<string, object?>
                        { ["id"] = e.Model?.Id.Entry, ["cost"] = e.Cost, ["stocked"] = e.IsStocked }).ToList(),
                    ["potions"] = inv.PotionEntries.Select(e => new Dictionary<string, object?>
                        { ["id"] = e.Model?.Id.Entry, ["cost"] = e.Cost, ["stocked"] = e.IsStocked }).ToList(),
                    ["removal"] = inv.CardRemovalEntry is { } rm
                        ? new Dictionary<string, object?> { ["cost"] = rm.Cost, ["used"] = rm.Used }
                        : null,
                };
            case MapRoom:
                return new()
                {
                    ["kind"] = "map",
                    ["next"] = NextNodes(run),
                };
            default:
                return null;
        }
    }

    // Coordinates the player may travel to from the current position.
    private static List<int[]> NextNodes(RunState run)
    {
        var map = run.Map;
        IEnumerable<MapPoint> next = run.CurrentMapCoord is { } cur && map.GetPoint(cur) is { } here
            ? here.Children
            : new[] { map.StartingMapPoint }.Concat(map.StartingMapPoint.Children);
        return next.OrderBy(p => p.coord.row).ThenBy(p => p.coord.col).Select(p => new[] { p.coord.col, p.coord.row }).ToList();
    }

    private static T Prop<T>(object o, string name) =>
        (T)o.GetType().GetProperty(name)!.GetValue(o)!;
}
