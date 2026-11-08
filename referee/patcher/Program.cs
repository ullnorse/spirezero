// Opt-in real-game branches for the referee (docs/specs/test-mode.md). The referee runs with TestMode.IsOn = true, so
// the `TestMode` checks that change gameplay never take their real-game branch. This rewrites just those call sites
// in sts2.dll so that an environment variable switches one site to the real-game branch:
//
//   MerchantPotionEntry.CalcCost   TestMode.IsOff -> STS2_REAL_SHOP is set     (shop potion price roll)
//   Cauldron.GenerateRewards       TestMode.IsOn  -> STS2_REAL_CAULDRON is unset (5 rolled PotionRewards)
//   CallingBell.GenerateRewards    TestMode.IsOn  -> STS2_REAL_BELL is unset     (Common/Uncommon/Rare RelicRewards)
//
// With the variables unset the patched game behaves exactly like the unpatched one. referee.py sets them from the
// actions file's real_shop_prices / real_cauldron / real_calling_bell keys.
//
// Speed (docs/referee-perf.md): MapPathPruning.PruneAndRepair returns at once when STS2_FORCED_FIGHT is set, which
// start_run does for a forced fight (referee.py's `forced_fight`). Pruning is most of a warm forced fight's cost and
// only changes the act map, which a forced fight never reads; it draws only from the map's own `act_N_map` Rng.
// Usage: dotnet run -- <sts2.dll>   (patches in place)
using System;
using System.IO;
using System.Linq;
using Mono.Cecil;
using Mono.Cecil.Cil;

var dllPath = args[0];
var resolver = new DefaultAssemblyResolver();
resolver.AddSearchDirectory(Path.GetDirectoryName(dllPath)!);
var module = ModuleDefinition.ReadModule(dllPath, new ReaderParameters { AssemblyResolver = resolver, ReadingMode = ReadingMode.Deferred });
var getEnv = module.ImportReference(typeof(Environment).GetMethod(nameof(Environment.GetEnvironmentVariable), new[] { typeof(string) })!);

// (type, method, TestMode getter, environment variable, true when the variable is set?)
var sites = new (string Type, string Method, string Getter, string Var, bool SetMeansTrue)[]
{
    ("MegaCrit.Sts2.Core.Entities.Merchant.MerchantPotionEntry", "CalcCost", "get_IsOff", "STS2_REAL_SHOP", true),
    ("MegaCrit.Sts2.Core.Models.Relics.Cauldron", "GenerateRewards", "get_IsOn", "STS2_REAL_CAULDRON", false),
    ("MegaCrit.Sts2.Core.Models.Relics.CallingBell", "GenerateRewards", "get_IsOn", "STS2_REAL_BELL", false),
};

foreach (var (typeName, methodName, getter, variable, setMeansTrue) in sites)
{
    var type = module.GetType(typeName) ?? throw new Exception($"type {typeName} not found");
    var method = type.Methods.Single(m => m.Name == methodName);
    var il = method.Body.GetILProcessor();
    var calls = method.Body.Instructions
        .Where(i => i.OpCode == OpCodes.Call && i.Operand is MethodReference r && r.DeclaringType.Name == "TestMode" && r.Name == getter)
        .ToList();
    if (calls.Count != 1) throw new Exception($"{typeName}.{methodName}: expected one TestMode.{getter} call, found {calls.Count}");
    // Rewrite in place so branches that target the call still land on the new sequence:
    //   ldstr VAR; call GetEnvironmentVariable; ldnull; cgt.un (set -> true) | ceq (unset -> true)
    var call = calls[0];
    call.OpCode = OpCodes.Ldstr;
    call.Operand = variable;
    var invoke = Instruction.Create(OpCodes.Call, getEnv);
    var ldnull = Instruction.Create(OpCodes.Ldnull);
    var compare = Instruction.Create(setMeansTrue ? OpCodes.Cgt_Un : OpCodes.Ceq);
    il.InsertAfter(call, invoke);
    il.InsertAfter(invoke, ldnull);
    il.InsertAfter(ldnull, compare);
    Console.WriteLine($"  Patched {type.Name}.{methodName}: TestMode.{getter} -> ${variable}");
}

{
    var type = module.GetType("MegaCrit.Sts2.Core.Map.MapPathPruning") ?? throw new Exception("MapPathPruning not found");
    var method = type.Methods.Single(m => m.Name == "PruneAndRepair");
    var il = method.Body.GetILProcessor();
    var first = method.Body.Instructions[0];
    // ldstr STS2_FORCED_FIGHT; call GetEnvironmentVariable; brfalse first; ret
    var ldstr = Instruction.Create(OpCodes.Ldstr, "STS2_FORCED_FIGHT");
    il.InsertBefore(first, ldstr);
    il.InsertAfter(ldstr, Instruction.Create(OpCodes.Call, getEnv));
    il.InsertBefore(first, Instruction.Create(OpCodes.Brfalse, first));
    il.InsertBefore(first, Instruction.Create(OpCodes.Ret));
    Console.WriteLine("  Patched MapPathPruning.PruneAndRepair: skipped when $STS2_FORCED_FIGHT is set");
}

var outPath = dllPath + ".patched";
module.Write(outPath);
File.Delete(dllPath);
File.Move(outPath, dllPath);
