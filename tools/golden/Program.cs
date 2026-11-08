// Golden-vector generator: calls the game's own Rng / StringHelper / RunRngSet / ListExtensions
// from sts2.dll (referenced by path, never copied) and writes compact, deterministic JSON fixtures.
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.Loader;
using System.Text;
using System.Text.Json;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Extensions;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves;

namespace Golden;

public static class Program
{
    public static int Main(string[] args)
    {
        if (args.Length != 1)
        {
            Console.Error.WriteLine("usage: dotnet run --project tools/golden -- <outdir>");
            return 1;
        }

        string gameDir = Environment.GetEnvironmentVariable("STS2_GAME")
            ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                ".local/share/Steam/steamapps/common/Slay the Spire 2/data_sts2_linuxbsd_x86_64");
        AssemblyLoadContext.Default.Resolving += (ctx, name) =>
        {
            string path = Path.Combine(gameDir, name.Name + ".dll");
            return File.Exists(path) ? ctx.LoadFromAssemblyPath(path) : null;
        };

        return Run(Path.GetFullPath(args[0]));
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static int Run(string outDir)
    {
        Directory.CreateDirectory(outDir);
        Fixtures.SeedHash(Path.Combine(outDir, "seed_hash.json"));
        Fixtures.RngVectors(Path.Combine(outDir, "rng.json"));
        int mismatches = Fixtures.Shuffle(Path.Combine(outDir, "shuffle.json"));
        return mismatches == 0 ? 0 : 2;
    }
}

/// <summary>Thin wrapper so every fixture is written the same compact way.</summary>
internal sealed class Json : IDisposable
{
    private readonly MemoryStream _ms = new();
    private readonly string _path;
    public readonly Utf8JsonWriter W;

    public Json(string path)
    {
        _path = path;
        W = new Utf8JsonWriter(_ms, new JsonWriterOptions { Indented = false });
    }

    public void Dispose()
    {
        W.Flush();
        _ms.WriteByte((byte)'\n');
        File.WriteAllBytes(_path, _ms.ToArray());
        Console.Error.WriteLine($"wrote {_path} ({_ms.Length} bytes)");
        W.Dispose();
    }

    /// <summary>Writes one value. Floating-point values are written as their raw IEEE-754 bits.</summary>
    public void Value(object v)
    {
        switch (v)
        {
            case null: W.WriteNullValue(); break;
            case bool b: W.WriteBooleanValue(b); break;
            case int i: W.WriteNumberValue(i); break;
            case uint u: W.WriteNumberValue(u); break;
            case long l: W.WriteNumberValue(l); break;
            case ulong ul: W.WriteNumberValue(ul); break;
            case double d: W.WriteNumberValue(BitConverter.DoubleToUInt64Bits(d)); break;
            case float f: W.WriteNumberValue(BitConverter.SingleToUInt32Bits(f)); break;
            case string s: W.WriteStringValue(s); break;
            default: throw new InvalidOperationException(v.GetType().ToString());
        }
    }

    public void State(SerializableRng s)
    {
        W.WriteStartObject();
        W.WriteNumber("counter", s.counter);
        W.WritePropertyName("s");
        W.WriteStartArray();
        W.WriteNumberValue(s.state0);
        W.WriteNumberValue(s.state1);
        W.WriteNumberValue(s.state2);
        W.WriteNumberValue(s.state3);
        W.WriteEndArray();
        W.WriteEndObject();
    }
}

internal static class Fixtures
{
    // Seeds taken from real saved runs (profile1/saves/history/*.run, field "seed"; 10 chars in this format).
    private static readonly string[] RunSeeds = { "94LYYA8YDC", "N68KUT7X8Q", "QY9GMDLPYN" };

    private static ulong Xxh64(string s) => StringHelper.GetDeterministicHashCode(s);

    // ---------------------------------------------------------------- seed_hash.json

    public static void SeedHash(string path)
    {
        using var j = new Json(path);
        var w = j.W;
        w.WriteStartObject();

        // xxh64 of strings (StringHelper.GetDeterministicHashCode = XxHash64 of UTF-8, seed 0).
        w.WritePropertyName("xxh64");
        w.WriteStartArray();
        string[] hashInputs =
        {
            "", "a", "abc", "0", "ABCDEFGHJKLM", "94LYYA8YDC", "N68KUT7X8Q", "QY9GMDLPYN",
            "0123456789ABCDEFGHJKLMNPQRSTUVWXYZ", // 34 bytes
            "The quick brown fox jumps over the lazy dog, twice: the quick brown fox jumps over the lazy dog", // > 64
            "Ünïcödé", "日本語", "🐍", "old12345", "up_front", "combat_orbs", "act_1_map",
        };
        foreach (string s in hashInputs)
        {
            w.WriteStartObject();
            w.WriteString("s", s);
            w.WriteNumber("h", Xxh64(s));
            w.WriteEndObject();
        }
        w.WriteEndArray();

        // SnakeCase (the game's own regex helper).
        w.WritePropertyName("snake_case");
        w.WriteStartArray();
        var names = new List<string>();
        names.AddRange(Enum.GetNames<RunRngType>());
        names.AddRange(Enum.GetNames<PlayerRngType>());
        names.AddRange(new[] { "CombatOrbGeneration", "ABc", "Act1Map", "  Padded  " });
        foreach (string n in names)
        {
            w.WriteStartObject();
            w.WriteString("s", n);
            w.WriteString("snake", StringHelper.SnakeCase(n));
            w.WriteEndObject();
        }
        w.WriteEndArray();

        // CanonicalizeSeed (user-typed seed normalisation).
        w.WritePropertyName("canonicalize");
        w.WriteStartArray();
        foreach (string s in new[] { "94lyya8ydc", " abcoiOI12 ", "o0i1", "Straße", "ROT0", "rot0", "OX3", "XO3", "I7", "L7", "18_10_2026_1p", "\u00a0Io\t", "ıiİ" })
        {
            w.WriteStartObject();
            w.WriteString("s", s);
            w.WriteString("canonical", SeedHelper.CanonicalizeSeed(s));
            w.WriteEndObject();
        }
        w.WriteEndArray();

        // Full RunRngSet per seed string. "stream_seed" = Seed + xxh64(name), the u64 handed to MegaRandom.
        w.WritePropertyName("seeds");
        w.WriteStartArray();
        var seedStrings = new List<(string, string)>();
        foreach (string s in RunSeeds) seedStrings.Add((s, "run_file"));
        seedStrings.Add(("ABCDEFGHJKLM", "synthetic_12_char"));
        seedStrings.Add(("TESTSEED", "synthetic"));
        seedStrings.Add(("", "synthetic_empty"));
        foreach (var (str, source) in seedStrings)
        {
            var set = new RunRngSet(str);
            var ser = set.ToSerializable();
            w.WriteStartObject();
            w.WriteString("seed_string", str);
            w.WriteString("source", source);
            w.WriteNumber("seed", set.Seed);
            w.WritePropertyName("streams");
            w.WriteStartArray();
            foreach (RunRngType t in Enum.GetValues<RunRngType>())
            {
                string name = StringHelper.SnakeCase(t.ToString());
                w.WriteStartObject();
                w.WriteString("type", t.ToString());
                w.WriteString("name", name);
                w.WriteNumber("stream_seed", set.Seed + Xxh64(name));
                w.WritePropertyName("initial");
                j.State(ser.Rngs[t]);
                w.WriteEndObject();
            }
            w.WriteEndArray();

            // PlayerRngSet(xxh64(seed) + slot) for each slot.
            w.WritePropertyName("players");
            w.WriteStartArray();
            for (ulong slot = 0; slot < 4; slot++)
            {
                var pset = new PlayerRngSet(set.Seed + slot);
                var pser = pset.ToSerializable();
                w.WriteStartObject();
                w.WriteNumber("slot", slot);
                w.WriteNumber("player_seed", pset.Seed);
                w.WritePropertyName("streams");
                w.WriteStartArray();
                foreach (PlayerRngType t in Enum.GetValues<PlayerRngType>())
                {
                    string name = StringHelper.SnakeCase(t.ToString());
                    w.WriteStartObject();
                    w.WriteString("type", t.ToString());
                    w.WriteString("name", name);
                    w.WriteNumber("stream_seed", pset.Seed + Xxh64(name));
                    w.WritePropertyName("initial");
                    j.State(pser.Rngs[t]);
                    w.WriteEndObject();
                }
                w.WriteEndArray();
                w.WriteEndObject();
            }
            w.WriteEndArray();
            w.WriteEndObject();
        }
        w.WriteEndArray();

        // new Rng(seed, name) for the dynamic stream names used by map generation etc.
        w.WritePropertyName("named_rng");
        w.WriteStartArray();
        string[] dynamicNames = { "act_1_map", "act_2_map", "act_3_map", "spoils_map", "map_point_selection", "event_synchronizer", "act_selection" };
        foreach (string str in RunSeeds)
        {
            ulong seed = Xxh64(str);
            foreach (string n in dynamicNames)
            {
                var rng = new Rng(seed, n);
                w.WriteStartObject();
                w.WriteNumber("seed", seed);
                w.WriteString("name", n);
                w.WritePropertyName("initial");
                j.State(rng.ToSerializable());
                w.WriteEndObject();
            }
        }
        w.WriteEndArray();

        w.WriteEndObject();
    }

    // ---------------------------------------------------------------- rng.json

    private static string FArg(float f) => $"{BitConverter.SingleToUInt32Bits(f)}";
    private static string DArg(double d) => $"{BitConverter.DoubleToUInt64Bits(d)}";

    private static void Series(Json j, ulong seed, string name, string kind, string args, int n,
        Func<Rng, object> f, bool statesAfter = false)
    {
        var w = j.W;
        var rng = new Rng(seed);
        w.WriteStartObject();
        w.WriteString("name", name);
        w.WriteString("kind", kind);
        w.WritePropertyName("args");
        w.WriteRawValue(args);
        w.WritePropertyName("out");
        w.WriteStartArray();
        var states = new List<SerializableRng>();
        for (int i = 0; i < n; i++)
        {
            j.Value(f(rng));
            if (statesAfter) states.Add(rng.ToSerializable());
        }
        w.WriteEndArray();
        if (statesAfter)
        {
            w.WritePropertyName("state_after");
            w.WriteStartArray();
            foreach (var s in states) j.State(s);
            w.WriteEndArray();
        }
        w.WritePropertyName("final");
        j.State(rng.ToSerializable());
        w.WriteEndObject();
    }

    private static void Errors(Json j, ulong seed)
    {
        var w = j.W;
        var cases = new (string, Action<Rng>)[]
        {
            ("NextInt(0)", r => r.NextInt(0)),
            ("NextInt(-1)", r => r.NextInt(-1)),
            ("NextInt(5,5)", r => r.NextInt(5, 5)),
            ("NextInt(6,5)", r => r.NextInt(6, 5)),
            ("NextUnsignedInt(5,5)", r => r.NextUnsignedInt(5, 5)),
            ("NextUnsignedLong(7,7)", r => r.NextUnsignedLong(7, 7)),
            ("NextFloat(2,1)", r => r.NextFloat(2f, 1f)),
            ("NextDouble(2,1)", r => r.NextDouble(2.0, 1.0)),
        };
        w.WriteStartArray();
        foreach (var (name, act) in cases)
        {
            var rng = new Rng(seed);
            string result = "ok";
            try { act(rng); }
            catch (Exception e) { result = e.GetType().Name; }
            w.WriteStartObject();
            w.WriteString("call", name);
            w.WriteString("result", result);
            w.WritePropertyName("final");
            j.State(rng.ToSerializable());
            w.WriteEndObject();
        }
        w.WriteEndArray();
    }

    private static readonly float[][] WeightSets =
    {
        new[] { 1f, 2f, 3f, 4f },
        new[] { 0.1f, 0.2f, 0.3f, 0.7f, 0.1f, 0.1f, 0.1f },
        new[] { 0.3f, 0.3f, 0.4f },
        new[] { 33f, 33f, 34f },
        new[] { 0f, 5f, 0f, 1f },
    };

    public static void RngVectors(string path)
    {
        using var j = new Json(path);
        var w = j.W;
        ulong[] seeds = { 0UL, 0xDEADBEEFCAFEBABEUL, Xxh64("94LYYA8YDC") };
        w.WriteStartObject();
        w.WriteString("note", "floats/doubles are raw IEEE-754 bits (f32 -> u32, f64 -> u64). Each series starts from a fresh Rng(seed).");
        w.WritePropertyName("seeds");
        w.WriteStartArray();
        for (int si = 0; si < seeds.Length; si++)
        {
            ulong seed = seeds[si];
            bool first = si == 0;
            w.WriteStartObject();
            w.WriteNumber("seed", seed);
            w.WritePropertyName("series");
            w.WriteStartArray();

            Series(j, seed, "NextUnsignedLong", "u64", "[]", 100, r => r.NextUnsignedLong(), statesAfter: first);
            Series(j, seed, "NextDouble", "f64", "[]", 100, r => r.NextDouble(), statesAfter: first);

            foreach (int max in new[] { 1, 2, 3, 7, 10, 52, 100, 1000, 65536, int.MaxValue })
                Series(j, seed, "NextInt", "i32", $"[{max}]", 40, r => r.NextInt(max));
            Series(j, seed, "NextInt_default", "i32", "[]", 40, r => r.NextInt());

            (int, int)[] ranges =
            {
                (0, 10), (-5, 5), (5, 6), (-100, -50), (3, 1000), (-1000000, 1000000),
                (0, int.MaxValue), (-1, int.MaxValue),            // n == 2^31: wide path
                (int.MinValue, int.MaxValue), (int.MinValue, 0),  // n > i32::MAX: wide path
            };
            foreach (var (a, b) in ranges)
                Series(j, seed, "NextInt_range", "i32", $"[{a},{b}]", 40, r => r.NextInt(a, b));

            Series(j, seed, "NextBool", "bool", "[]", 40, r => r.NextBool());

            foreach (var (a, b) in new[] { (0.0, 1.0), (-1.0, 1.0), (10.0, 20.0), (0.0, 0.1) })
                Series(j, seed, "NextDouble_range", "f64", $"[{DArg(a)},{DArg(b)}]", 40, r => r.NextDouble(a, b));

            Series(j, seed, "NextFloat", "f32", "[]", 40, r => r.NextFloat());
            foreach (float x in new[] { 0.5f, 1f, 10f, 100f, 0.3f + 0.3f + 0.4f, 0.1f + 0.2f + 0.3f + 0.7f + 0.1f + 0.1f + 0.1f, 16777216f, 3.4028235E38f })
                Series(j, seed, "NextFloat_max", "f32", $"[{FArg(x)}]", 40, r => r.NextFloat(x));
            foreach (var (a, b) in new[] { (0f, 1f), (-1f, 1f), (0.1f, 0.7f), (-50f, 50f), (1e7f, 1e7f + 8f), (-3.4e38f, 3.4e38f) })
                Series(j, seed, "NextFloat_range", "f32", $"[{FArg(a)},{FArg(b)}]", 40, r => r.NextFloat(a, b));

            foreach (uint max in new uint[] { 1, 10, 100, 65536, uint.MaxValue })
                Series(j, seed, "NextUnsignedInt", "u32", $"[{max}]", 40, r => r.NextUnsignedInt(max));
            Series(j, seed, "NextUnsignedInt_default", "u32", "[]", 40, r => r.NextUnsignedInt());
            foreach (var (a, b) in new[] { (5u, 6u), (10u, 100u), (1000u, uint.MaxValue), (0u, uint.MaxValue) })
                Series(j, seed, "NextUnsignedInt_range", "u32", $"[{a},{b}]", 40, r => r.NextUnsignedInt(a, b));

            foreach (ulong max in new[] { 1UL, 100UL, 1UL << 32, 1UL << 53, (1UL << 53) + 12345, ulong.MaxValue - 1, ulong.MaxValue })
                Series(j, seed, "NextUnsignedLong_max", "u64", $"[{max}]", 40, r => r.NextUnsignedLong(max));
            foreach (var (a, b) in new[] { (5UL, 6UL), (100UL, 200UL), (1UL << 40, 1UL << 62), (0UL, ulong.MaxValue) })
                Series(j, seed, "NextUnsignedLong_range", "u64", $"[{a},{b}]", 40, r => r.NextUnsignedLong(a, b));

            int[] ints = { 10, 20, 30, 40, 50, 60, 70 };
            Series(j, seed, "NextItem", "i32", $"[[{string.Join(",", ints)}]]", 40, r => r.NextItem(ints));
            Series(j, seed, "NextItem_single", "i32", "[[99]]", 10, r => r.NextItem(new[] { 99 }));
            Series(j, seed, "NextItem_empty", "i32", "[[]]", 5, r => r.NextItem(Array.Empty<int>()));

            // Instance WeightedNextItem: draws NextFloat() first, then reads weights. Items are 1..n; fallback is 0.
            foreach (float[] ws in WeightSets)
            {
                int[] items = Enumerable.Range(1, ws.Length).ToArray();
                Series(j, seed, "WeightedNextItem", "i32", $"[[{string.Join(",", ws.Select(FArg))}]]", 40,
                    r => r.WeightedNextItem(items, i => ws[i - 1]));
            }

            // Gaussians: Math.Log/Cos/Sin go through the host libm, so these may differ in the last ulp between platforms.
            Series(j, seed, "NextGaussianDouble", "f64", $"[{DArg(0.5)},{DArg(0.2)},{DArg(0.0)},{DArg(1.0)}]", 40, r => r.NextGaussianDouble(0.5, 0.2, 0.0, 1.0));
            Series(j, seed, "NextGaussianDouble", "f64", $"[{DArg(0.3)},{DArg(0.5)},{DArg(-2.0)},{DArg(8.0)}]", 40, r => r.NextGaussianDouble(0.3, 0.5, -2.0, 8.0));
            Series(j, seed, "NextGaussianInt", "i32", "[3,1,0,6]", 40, r => r.NextGaussianInt(3, 1, 0, 6));
            Series(j, seed, "NextGaussianInt", "i32", "[7,3,0,14]", 40, r => r.NextGaussianInt(7, 3, 0, 14));

            // Rng.Shuffle (instance) on identity lists.
            foreach (int len in new[] { 0, 1, 2, 5, 10, 52 })
            {
                var list = Enumerable.Range(0, len).ToList();
                var rng = new Rng(seed);
                rng.Shuffle(list);
                w.WriteStartObject();
                w.WriteString("name", "Rng.Shuffle");
                w.WriteString("kind", "i32_list");
                w.WritePropertyName("args");
                w.WriteRawValue($"[{len}]");
                w.WritePropertyName("out");
                w.WriteStartArray();
                foreach (int v in list) w.WriteNumberValue(v);
                w.WriteEndArray();
                w.WritePropertyName("final");
                j.State(rng.ToSerializable());
                w.WriteEndObject();
            }

            w.WriteEndArray();
            w.WritePropertyName("errors");
            Errors(j, seed);
            w.WriteEndObject();
        }
        w.WriteEndArray();

        // Static WeightedNextItem(randInput, ...) with chosen inputs: exercises the f32 sum / subtraction exactly.
        w.WritePropertyName("weighted_static");
        w.WriteStartArray();
        var inputs = new List<float>();
        for (int i = 0; i <= 100; i++) inputs.Add(i / 100f);
        inputs.Add(MathF.BitDecrement(1f));
        inputs.Add(1f / 3f);
        inputs.Add(2f / 3f);
        foreach (float[] ws in WeightSets)
        {
            int[] items = Enumerable.Range(1, ws.Length).ToArray();
            w.WriteStartObject();
            w.WritePropertyName("weights");
            w.WriteStartArray();
            foreach (float x in ws) j.Value(x);
            w.WriteEndArray();
            w.WritePropertyName("inputs");
            w.WriteStartArray();
            foreach (float x in inputs) j.Value(x);
            w.WriteEndArray();
            w.WritePropertyName("out");
            w.WriteStartArray();
            foreach (float x in inputs) w.WriteNumberValue(Rng.WeightedNextItem(x, items, i => ws[i - 1], 0));
            w.WriteEndArray();
            w.WriteEndObject();
        }
        w.WriteEndArray();

        w.WriteEndObject();
    }

    // ---------------------------------------------------------------- shuffle.json

    private static void IntList(Json j, string name, IEnumerable<int> xs)
    {
        j.W.WritePropertyName(name);
        j.W.WriteStartArray();
        foreach (int x in xs) j.W.WriteNumberValue(x);
        j.W.WriteEndArray();
    }

    /// <summary>Key patterns for tie tests; each returns the key of element i (n elements).</summary>
    private static readonly (string, Func<int, int, int>)[] KeyPatterns =
    {
        ("all_equal", (i, n) => 0),
        ("mod2", (i, n) => i % 2),
        ("mod3_sq", (i, n) => (i * i + 3 * i) % 3),
        ("mod5", (i, n) => (i * 7 + 1) % 5),
        ("desc", (i, n) => n - i),
        ("pairs", (i, n) => i / 2),
        ("blocks", (i, n) => (i / 5) % 3),
    };

    public static int Shuffle(string path)
    {
        using var j = new Json(path);
        var w = j.W;
        int[] sizes = { 0, 1, 5, 16, 17, 40 };
        int[] tieSizes = { 0, 1, 2, 3, 5, 16, 17, 24, 40, 64, 100 };
        ulong[] seeds = { 1UL, 987654321UL, Xxh64("94LYYA8YDC") };
        int mismatches = 0;
        int checkedCases = 0;

        w.WriteStartObject();
        w.WriteString("note", "int lists: values; tie lists: elements are tags (original index), ordered by key only.");

        // Plain int lists (no ties): UnstableShuffle / StableShuffle from identity and from a scrambled input.
        w.WritePropertyName("int_lists");
        w.WriteStartArray();
        foreach (ulong seed in seeds)
        foreach (int n in sizes)
        {
            var scrambled = Enumerable.Range(0, n).ToList();
            new Rng(12345).Shuffle(scrambled);

            var unstable = Enumerable.Range(0, n).ToList();
            var rngU = new Rng(seed);
            unstable.UnstableShuffle(rngU);

            var stableId = Enumerable.Range(0, n).ToList();
            var rngS1 = new Rng(seed);
            stableId.StableShuffle(rngS1);

            var stableScr = scrambled.ToList();
            var rngS2 = new Rng(seed);
            stableScr.StableShuffle(rngS2);

            w.WriteStartObject();
            w.WriteNumber("seed", seed);
            w.WriteNumber("n", n);
            IntList(j, "scrambled_input", scrambled);
            IntList(j, "unstable_identity", unstable);
            IntList(j, "stable_identity", stableId);
            IntList(j, "stable_scrambled", stableScr);
            w.WritePropertyName("final");
            j.State(rngU.ToSerializable());
            w.WriteEndObject();
        }
        w.WriteEndArray();

        // Tie lists: capture List<T>.Sort() tie order (introsort) and both shuffles.
        w.WritePropertyName("tie_lists");
        w.WriteStartArray();
        foreach (var (pname, keyFn) in KeyPatterns)
        foreach (int n in tieSizes)
        {
            Tied[] Make() => Enumerable.Range(0, n).Select(i => new Tied(keyFn(i, n), i)).ToArray();

            var sorted = Make().ToList();
            sorted.Sort();
            checkedCases++;
            mismatches += IntroSortCheck.Verify(Make(), sorted);

            w.WriteStartObject();
            w.WriteString("pattern", pname);
            w.WriteNumber("n", n);
            IntList(j, "keys", Make().Select(t => t.Key));
            IntList(j, "sort_only", sorted.Select(t => t.Tag));

            w.WritePropertyName("shuffles");
            w.WriteStartArray();
            foreach (ulong seed in seeds)
            {
                var u = Make().ToList();
                var ru = new Rng(seed);
                u.UnstableShuffle(ru);
                var s = Make().ToList();
                var rs = new Rng(seed);
                s.StableShuffle(rs);
                w.WriteStartObject();
                w.WriteNumber("seed", seed);
                IntList(j, "unstable", u.Select(t => t.Tag));
                IntList(j, "stable", s.Select(t => t.Tag));
                w.WritePropertyName("final");
                j.State(rs.ToSerializable());
                w.WriteEndObject();
            }
            w.WriteEndArray();
            w.WriteEndObject();
        }
        w.WriteEndArray();
        w.WriteEndObject();

        // Extra confirmation of the spec's introsort description against the real List<T>.Sort(), not written out.
        var rnd = new Random(2024);
        for (int round = 0; round < 3000; round++)
        {
            int n = rnd.Next(0, 300);
            int range = rnd.Next(1, 12);
            var keys = new int[n];
            for (int i = 0; i < n; i++) keys[i] = rnd.Next(range);
            Tied[] Make() => Enumerable.Range(0, n).Select(i => new Tied(keys[i], i)).ToArray();
            var real = Make().ToList();
            real.Sort();
            checkedCases++;
            mismatches += IntroSortCheck.Verify(Make(), real);
        }
        Console.Error.WriteLine($"introsort-check: {checkedCases} cases, {mismatches} mismatches");
        return mismatches;
    }
}
