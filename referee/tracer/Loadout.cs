// `set_loadout` command compiled into the vendored sts2-cli (see referee/setup.sh).
// Replaces the player's deck / relics / potions / HP through the game's own add paths, before the
// forced room is entered. Unlike sts2-cli's `set_player` it validates every id first and fails loudly.
using System.Text.Json;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

public static class Loadout
{
    public static Dictionary<string, object?> Apply(JsonElement cmd)
    {
        try
        {
            var run = RunManager.Instance.DebugOnlyGetState();
            if (run == null) return Error("No run in progress");
            var player = run.Players[0];

            // Resolve everything before touching state, so a bad id leaves the run untouched.
            var deck = Resolve<CardModel>(cmd, "deck", "CARD", out var deckUpgrades, out var deckEnchants, out var deckCounters);
            var relics = Resolve<RelicModel>(cmd, "relics", "RELIC", out _, out _, out _);
            var potions = Resolve<PotionModel>(cmd, "potions", "POTION", out _, out _, out _);
            if (potions != null && potions.Count > player.PotionSlots.Count)
                throw new ArgumentException($"{potions.Count} potions do not fit in {player.PotionSlots.Count} slots");

            if (deck != null)
            {
                foreach (var c in player.Deck.Cards.ToList()) run.RemoveCard(c);
                player.Deck.Clear(silent: true);
                for (int i = 0; i < deck.Count; i++)
                {
                    var card = run.CreateCard(deck[i], player);
                    for (int u = 0; u < deckUpgrades[i] && card.IsUpgradable; u++)
                    {
                        card.UpgradeInternal();
                        card.FinalizeUpgradeInternal();
                    }
                    if (deckEnchants[i] is var (enchantment, amount))
                        CardCmd.Enchant(enchantment.ToMutable(), card, amount);
                    SetCounter(card, deckCounters[i]);
                    player.Deck.AddInternal(card);
                }
            }

            if (relics != null)
            {
                foreach (var r in player.Relics.ToList()) player.RemoveRelicInternal(r, silent: true);
                foreach (var r in relics)
                    RelicCmd.Obtain(r.ToMutable(), player).GetAwaiter().GetResult();
            }

            if (potions != null)
            {
                foreach (var p in potions)
                {
                    var res = PotionCmd.TryToProcure(p.ToMutable(), player).GetAwaiter().GetResult();
                    if (!res.success) throw new ArgumentException($"potion {p.Id.Entry} not procured: {res.failureReason}");
                }
            }

            // Last, so explicit values win over anything a relic's AfterObtained did to max HP.
            if (cmd.TryGetProperty("max_hp", out var mhp)) player.Creature.SetMaxHpInternal(mhp.GetInt32());
            if (cmd.TryGetProperty("hp", out var hp)) player.Creature.SetCurrentHpInternal(hp.GetInt32());

            return new Dictionary<string, object?> { ["type"] = "ok" };
        }
        catch (Exception ex)
        {
            return Error($"{ex.GetType().Name}: {ex.Message}");
        }
    }

    // Sets the saved counter of a deck card: `Guilty.CombatsSeen` or `Dowsing.RoomsEntered`.
    internal static void SetCounter(CardModel card, int? counter)
    {
        switch (counter, card)
        {
            case (null, _):
                break;
            case (int n, MegaCrit.Sts2.Core.Models.Cards.Guilty guilty):
                guilty.CombatsSeen = n;
                break;
            case (int n, MegaCrit.Sts2.Core.Models.Cards.Dowsing dowsing):
                dowsing.RoomsEntered = n;
                break;
            default:
                throw new ArgumentException($"{card.Id.Entry} has no counter");
        }
    }

    // Looks ids up as ["ID", "ID+1", "ID+1:ENCHANTMENT", "ID:ENCHANTMENT*2", "GUILTY#2", ...]; "+N" is the number of
    // upgrades, ":E*A" enchants a card with E (`CardCmd.Enchant`, amount A, default 1), a final "#N" sets the
    // saved counter (Guilty's `CombatsSeen`, Dowsing's `RoomsEntered`). Null when the field is absent.
    internal static List<T>? Resolve<T>(JsonElement cmd, string field, string category, out List<int> upgrades,
        out List<(EnchantmentModel, int)?> enchants, out List<int?> counters)
        where T : AbstractModel
    {
        upgrades = new();
        enchants = new();
        counters = new();
        if (!cmd.TryGetProperty(field, out var arr)) return null;
        var models = new List<T>();
        foreach (var el in arr.EnumerateArray())
        {
            var raw = el.GetString() ?? throw new ArgumentException($"{field}: null id");
            (EnchantmentModel, int)? enchant = null;
            int? counter = null;
            var hash = raw.IndexOf('#');
            if (hash >= 0)
            {
                if (category != "CARD") throw new ArgumentException($"{field}: '{raw}' has no counter");
                counter = int.Parse(raw[(hash + 1)..]);
                raw = raw[..hash];
            }
            var colon = raw.IndexOf(':');
            if (colon >= 0)
            {
                if (category != "CARD") throw new ArgumentException($"{field}: '{raw}' cannot be enchanted");
                var spec = raw[(colon + 1)..];
                raw = raw[..colon];
                var star = spec.IndexOf('*');
                var enchantId = star < 0 ? spec : spec[..star];
                var amount = star < 0 ? 1 : int.Parse(spec[(star + 1)..]);
                var enchantment = ModelDb.GetByIdOrNull<EnchantmentModel>(
                        new ModelId(ModelId.SlugifyCategory<EnchantmentModel>(), enchantId))
                    ?? throw new ArgumentException($"unknown enchantment id '{enchantId}' in {field}");
                enchant = (enchantment, amount);
            }
            var plus = raw.IndexOf('+');
            var id = plus < 0 ? raw : raw[..plus];
            var level = plus < 0 ? 0 : int.Parse(raw[(plus + 1)..]);
            if (level > 0 && category != "CARD") throw new ArgumentException($"{field}: '{raw}' cannot be upgraded");
            var model = ModelDb.GetByIdOrNull<T>(new ModelId(category, id))
                ?? throw new ArgumentException($"unknown {category.ToLowerInvariant()} id '{id}' in {field}");
            models.Add(model);
            upgrades.Add(level);
            enchants.Add(enchant);
            counters.Add(counter);
        }
        return models;
    }

    internal static Dictionary<string, object?> Error(string message) =>
        new() { ["type"] = "error", ["message"] = message };
}
