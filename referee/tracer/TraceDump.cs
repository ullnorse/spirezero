// Read-only state dumper compiled into the vendored sts2-cli (see referee/setup.sh).
// Reads the live game objects through their public API; it never mutates game state.
using System.Reflection;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Localization.DynamicVars;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.MonsterMoves.Intents;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

public static class TraceDump
{
    public static Dictionary<string, object?> Dump()
    {
        var run = RunManager.Instance.DebugOnlyGetState();
        if (run == null) return new() { ["type"] = "error", ["message"] = "No run in progress" };

        var result = new Dictionary<string, object?>
        {
            ["type"] = "state",
            ["seed"] = run.Rng.StringSeed,
            ["act"] = run.Act.Id.Entry,
            ["floor"] = run.ActFloor,
            ["first_encounters"] = FirstEncounters(run.Act),
        };
        // Call counts of every run and player RNG stream (shared with dump_run).
        result["rng"] = RunDump.Rngs(run, run.Players[0]);

        var combat = CombatManager.Instance.DebugOnlyGetState();
        var player = run.Players[0];
        var pcs = player.PlayerCombatState;
        bool inProgress = CombatManager.Instance.IsInProgress;
        result["combat_over"] = combat == null || !inProgress;
        result["player_dead"] = player.Creature.IsDead;
        if (combat == null || pcs == null) return result;

        result["round"] = combat.RoundNumber;
        result["side"] = combat.CurrentSide.ToString();
        result["player"] = new Dictionary<string, object?>
        {
            ["hp"] = player.Creature.CurrentHp,
            ["max_hp"] = player.Creature.MaxHp,
            ["block"] = player.Creature.Block,
            ["energy"] = pcs.Energy,
            ["stars"] = pcs.Stars,
            ["base_orb_slots"] = player.BaseOrbSlotCount,
            ["orb_slots"] = pcs.OrbQueue.Capacity,
            ["orbs"] = pcs.OrbQueue.Orbs.Select(o => new Dictionary<string, object?>
            {
                ["id"] = o.Id.Entry,
                ["passive"] = o.PassiveVal,
                ["evoke"] = o.EvokeVal,
            }).ToList(),
            ["gold"] = player.Gold,
            ["max_energy"] = player.MaxEnergy,
            ["turn_number"] = pcs.TurnNumber,
            ["pets"] = pcs.Pets.Select(p => p.Monster?.Id.Entry).ToList(),
            ["relics"] = player.Relics.Select(Relic).ToList(),
            ["powers"] = Powers(player.Creature),
            ["potions"] = player.PotionSlots.Select(p => p?.Id.Entry).ToList(),
        };
        var osty = player.Osty;
        result["osty"] = osty == null
            ? new Dictionary<string, object?> { ["alive"] = false }
            : new Dictionary<string, object?>
            {
                ["alive"] = osty.IsAlive,
                ["hp"] = osty.CurrentHp,
                ["max_hp"] = osty.MaxHp,
                ["block"] = osty.Block,
                ["powers"] = Powers(osty),
            };
        result["hand"] = Cards(pcs.Hand.Cards);
        result["draw"] = Cards(pcs.DrawPile.Cards);
        result["discard"] = Cards(pcs.DiscardPile.Cards);
        result["exhaust"] = Cards(pcs.ExhaustPile.Cards);
        result["play"] = Cards(pcs.PlayPile.Cards);
        result["enemies"] = combat.Enemies.Select(e => new Dictionary<string, object?>
        {
            ["id"] = e.Monster?.Id.Entry,
            ["alive"] = e.IsAlive,
            ["hp"] = e.CurrentHp,
            ["max_hp"] = e.MaxHp,
            ["block"] = e.Block,
            ["next_move"] = e.Monster?.NextMove?.Id,
            ["spawned_this_turn"] = e.Monster?.SpawnedThisTurn,
            ["performed_first_move"] = e.Monster?.MoveStateMachine is { } m
                ? typeof(MegaCrit.Sts2.Core.MonsterMoves.MonsterMoveStateMachine.MonsterMoveStateMachine)
                    .GetField("_performedFirstMove", BindingFlags.Instance | BindingFlags.NonPublic)?.GetValue(m)
                : null,
            ["move_log"] = e.Monster?.MoveStateMachine?.StateLog.Select(st => st.Id).ToList(),
            ["intents"] = Intents(combat, e),
            ["powers"] = Powers(e),
        }).ToList();
        return result;
    }

    // The attack intents of the next move (`AttackIntent`s only; the other intent types carry no
    // numbers): `IntentType`, `GetSingleDamage` against the players and the hit count (`Repeats` of a
    // `MultiAttackIntent`, else 1). XT3.
    private static List<Dictionary<string, object?>> Intents(CombatState combat, Creature e)
    {
        if (e.Monster?.NextMove is not { } move) return new();
        var targets = combat.PlayerCreatures;
        return move.Intents.OfType<AttackIntent>().Select(i => new Dictionary<string, object?>
        {
            ["type"] = i.IntentType.ToString(),
            ["damage"] = i.GetSingleDamage(targets, e),
            ["hits"] = i is MultiAttackIntent ? i.Repeats : 1,
        }).ToList();
    }

    // `{id, amount, amount_on_turn_start, skip_tick, applier, target}`, plus `vars` (the `DynamicVars` whose
    // `BaseValue` differs from the canonical instance's, `[name, value]`) and `fields` (the power's own fields and
    // its `_internalData`'s, see ModelFields) when it has any (XT3).
    private static List<Dictionary<string, object?>> Powers(Creature c) =>
        c.Powers.Select(p =>
        {
            var power = new Dictionary<string, object?>
            {
                ["id"] = p.Id.Entry,
                ["amount"] = p.Amount,
                ["amount_on_turn_start"] = p.AmountOnTurnStart,
                ["skip_tick"] = p.SkipNextDurationTick,
                ["applier"] = ModelFields.CreatureRef(p.Applier),
                ["target"] = ModelFields.CreatureRef(p.Target),
            };
            var vars = ModelFields.ChangedVars(p.DynamicVars, typeof(PowerModel), p);
            if (vars.Count > 0) power["vars"] = vars;
            var data = typeof(PowerModel).GetField("_internalData", BindingFlags.Instance | BindingFlags.NonPublic)?.GetValue(p);
            var fields = ModelFields.Of(p, typeof(PowerModel), data);
            if (fields.Count > 0) power["fields"] = fields;
            return power;
        }).ToList();

    // A relic: `{id, vars, fields}` (`vars` as a power's, only when one changed; `fields` only when the relic
    // declares any).
    internal static Dictionary<string, object?> Relic(RelicModel r)
    {
        var relic = new Dictionary<string, object?> { ["id"] = r.Id.Entry };
        var vars = ModelFields.ChangedVars(r.DynamicVars, typeof(RelicModel), r);
        if (vars.Count > 0) relic["vars"] = vars;
        var fields = ModelFields.Of(r, typeof(RelicModel), null);
        if (fields.Count > 0) relic["fields"] = fields;
        return relic;
    }

    // `enchantment` ({id, amount, status, fields}), `affliction` ({id, amount}) and `fields` (the card class's own
    // fields, see ModelFields) are listed only when present. `vars` and `replay` are always listed (T15), and so
    // are `star_cost` (`GetStarCostWithModifiers`), `last_stars` (`LastStarsSpent`) and `clone` (`IsClone`) (XT3).
    private static List<Dictionary<string, object?>> Cards(IEnumerable<CardModel> cards) =>
        cards.Select(c =>
        {
            var card = new Dictionary<string, object?>
            {
                ["id"] = c.Id.Entry,
                ["upgrade"] = c.CurrentUpgradeLevel,
                ["cost"] = c.EnergyCost.GetAmountToSpend(),
                ["keywords"] = c.Keywords.Select(k => k.ToString()).Where(k => k != "None").OrderBy(k => k, StringComparer.Ordinal).ToList(),
                ["exhaust_on_next_play"] = c.ExhaustOnNextPlay,
                ["retain"] = c.ShouldRetainThisTurn,
                ["sly"] = c.IsSlyThisTurn,
                ["vars"] = Vars(c),
                ["replay"] = c.BaseReplayCount,
                ["star_cost"] = c.GetStarCostWithModifiers(),
                ["last_stars"] = c.LastStarsSpent,
                ["clone"] = c.IsClone,
            };
            if (c.Enchantment != null)
            {
                var enchantment = new Dictionary<string, object?>
                {
                    ["id"] = c.Enchantment.Id.Entry,
                    ["amount"] = c.Enchantment.Amount,
                    ["status"] = c.Enchantment.Status.ToString(),
                };
                var enchantmentFields = ModelFields.Of(c.Enchantment, typeof(EnchantmentModel), null);
                if (enchantmentFields.Count > 0) enchantment["fields"] = enchantmentFields;
                card["enchantment"] = enchantment;
            }
            if (c.Affliction != null)
                card["affliction"] = new Dictionary<string, object?> { ["id"] = c.Affliction.Id.Entry, ["amount"] = c.Affliction.Amount };
            var fields = ModelFields.Of(c, typeof(CardModel), null);
            if (fields.Count > 0) card["fields"] = fields;
            return card;
        }).ToList();

    // `DynamicVars` in their order (`CanonicalVars`, a dictionary that is never added to after construction):
    // `[name, BaseValue]`, plus `Calculate(null)` for a `CalculatedVar` (the value without a target). `PreviewValue`
    // is not dumped: only the UI updates it. A decimal is a JSON integer when integral, else its normalized string.
    internal static List<List<object?>> Vars(CardModel card) =>
        card.DynamicVars.Values.Select(v =>
        {
            var entry = new List<object?> { v.Name, Number(v.BaseValue) };
            if (v is CalculatedVar calculated)
            {
                try { entry.Add(Number(calculated.Calculate(null))); }
                catch (Exception e) { entry.Add("error: " + e.GetType().Name); }
            }
            return entry;
        }).ToList();

    private static object Number(decimal value) =>
        value == decimal.Truncate(value)
            ? (long)value
            : value.ToString("0.############################", System.Globalization.CultureInfo.InvariantCulture);

    // The pre-rolled normal-encounter list is fixed by the seed (plus unlock state).
    private static List<string> FirstEncounters(MegaCrit.Sts2.Core.Models.ActModel act)
    {
        var rooms = typeof(MegaCrit.Sts2.Core.Models.ActModel)
            .GetField("_rooms", BindingFlags.Instance | BindingFlags.NonPublic)?.GetValue(act) as RoomSet;
        return rooms?.normalEncounters.Take(3).Select(e => e.Id.Entry).ToList() ?? new();
    }
}

// The per-instance state a model subclass keeps in its own fields (XT3): every instance field declared
// between the model's concrete type and `stop` (CardModel, PowerModel, ...), plus those of `extra` (a power's
// `_internalData`), as `[name, value]` sorted by name. Names drop the leading `_` (an auto-property's backing
// field is named after the property) and are snake_case. Values: numbers, bools and enums (by name) as
// themselves, a card/power/relic/potion as its id, a creature as CreatureRef, a ModelId as its entry, a
// collection as its count, any other object as whether it is set. UI-only state is skipped: strings, floats,
// Godot and node types, hover tips, `_isActivating` (the flash animation), SovereignBlade's
// `_createdThroughForge` (VFX) and test/mock fields. A collection field still null counts as empty (0).
internal static class ModelFields
{
    private static readonly Dictionary<Type, List<FieldInfo>> Cache = new();

    public static List<List<object?>> Of(object model, Type stop, object? extra)
    {
        var fields = Declared(model.GetType(), stop).Select(f => (f, model));
        if (extra != null) fields = fields.Concat(Declared(extra.GetType(), typeof(object)).Select(f => (f, extra)));
        return fields
            .Select(x => new List<object?> { Name(x.f.Name), FieldValue(x.f, x.Item2) })
            .OrderBy(e => (string)e[0]!, StringComparer.Ordinal)
            .ToList();
    }

    private static List<FieldInfo> Declared(Type type, Type stop)
    {
        lock (Cache)
        {
            if (Cache.TryGetValue(type, out var cached)) return cached;
            var list = new List<FieldInfo>();
            for (var t = type; t != null && t != stop && t != typeof(object); t = t.BaseType)
                list.AddRange(t.GetFields(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.DeclaredOnly)
                    .Where(Kept));
            Cache[type] = list;
            return list;
        }
    }

    private static bool Kept(FieldInfo f)
    {
        var name = f.Name.TrimStart('_');
        // `SovereignBlade._createdThroughForge` only picks the blade's VFX (`AfterCardChangedPiles`).
        if (name is "isActivating" or "createdThroughForge" || name.StartsWith("test") || name.StartsWith("mock") || name.StartsWith("vfx")) return false;
        var t = Nullable.GetUnderlyingType(f.FieldType) ?? f.FieldType;
        if (t == typeof(string) || t == typeof(float) || t == typeof(double)) return false;
        if (typeof(Delegate).IsAssignableFrom(t)) return false;
        var ns = t.Namespace ?? "";
        if (ns.StartsWith("Godot") || ns.Contains(".Nodes") || ns.Contains("HoverTips") || ns.Contains("Localization")) return false;
        if (t.IsGenericType && t.GetGenericArguments().Any(a => (a.Namespace ?? "").Contains("HoverTips"))) return false;
        return true;
    }

    // `[name, BaseValue]` of each of `vars` whose value differs from the canonical instance's (its private
    // `CanonicalInstance`), in declaration order.
    public static List<List<object?>> ChangedVars(DynamicVarSet vars, Type modelType, AbstractModel model)
    {
        var canonical = modelType.GetProperty("CanonicalInstance", BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic)?.GetValue(model) as AbstractModel;
        var canonicalVars = canonical == null ? null
            : modelType.GetProperty("DynamicVars", BindingFlags.Instance | BindingFlags.Public)?.GetValue(canonical) as DynamicVarSet;
        return vars.Values
            .Where(v => canonicalVars == null || !canonicalVars.Values.Any(c => c.Name == v.Name && c.BaseValue == v.BaseValue))
            .Select(v => new List<object?> { v.Name, Value(v.BaseValue) })
            .ToList();
    }

    private static string Name(string raw)
    {
        var name = raw.StartsWith("<") ? raw[1..raw.IndexOf('>')] : raw.TrimStart('_');
        var sb = new System.Text.StringBuilder();
        for (int i = 0; i < name.Length; i++)
        {
            if (char.IsUpper(name[i]) && i > 0) sb.Append('_');
            sb.Append(char.ToLowerInvariant(name[i]));
        }
        return sb.ToString();
    }

    // A collection field that is still null counts as empty: such fields are created on first use
    // (`UnsettlingLamp._doubledPowers`).
    private static object? FieldValue(FieldInfo f, object owner) =>
        f.GetValue(owner) is { } v ? Value(v)
            : IsCollection(f.FieldType) ? 0 : null;

    // `ICollection`, or a generic `ICollection<T>` only (`HashSet<T>`, `HellraiserPower.Data.autoPlayingCards`).
    private static bool IsCollection(Type t) =>
        typeof(System.Collections.ICollection).IsAssignableFrom(t) || GenericCollection(t) != null;

    private static Type? GenericCollection(Type t) =>
        (t.IsInterface && t.IsGenericType && t.GetGenericTypeDefinition() == typeof(ICollection<>) ? new[] { t } : t.GetInterfaces())
            .FirstOrDefault(i => i.IsGenericType && i.GetGenericTypeDefinition() == typeof(ICollection<>));

    private static object? GenericCount(object v) =>
        GenericCollection(v.GetType()) is { } c ? c.GetProperty("Count")!.GetValue(v) : true;

    private static object? Value(object? v) => v switch
    {
        null => null,
        bool or int or long or uint => v,
        decimal d => d == decimal.Truncate(d)
            ? (long)d
            : d.ToString("0.############################", System.Globalization.CultureInfo.InvariantCulture),
        Enum e => e.ToString(),
        AbstractModel m => m.Id.Entry,
        Creature c => CreatureRef(c),
        Player => "player",
        ModelId id => id.Entry,
        System.Collections.ICollection c => c.Count,
        _ => GenericCount(v),
    };

    // A creature as `player`, `pet:<id>` or `enemy:<index in CombatState.Enemies>`; null for none.
    public static string? CreatureRef(Creature? c)
    {
        if (c == null) return null;
        if (c.IsPlayer) return "player";
        if (c.PetOwner != null) return "pet:" + c.Monster?.Id.Entry;
        var enemies = CombatManager.Instance.DebugOnlyGetState()?.Enemies;
        int index = enemies == null ? -1 : enemies.ToList().IndexOf(c);
        return "enemy:" + index;
    }
}
