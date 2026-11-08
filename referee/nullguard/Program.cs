// Null-safe Godot node singletons for the headless referee. Headless there is no scene tree, so every node singleton
// (`NCombatRoom.Instance`, `NAudioManager.Instance`, `NRun.Instance`, ...) is null, and game logic that dereferences one
// without `?.` throws: KaiserCrabBoss's arms call `NAudioManager.Instance.PlayOneShot` in `BeforeDeath`, so the throw
// skips `AfterDeath` and CrabRage never fires; `SoulNexus.AfterDeath` calls `NCombatRoom.Instance.GetCreatureNode` and
// the kill stalls. In the real game these singletons exist and the calls only drive visuals and audio.
//
// For each such dereference in a game-logic method (the namespaces below), the member access on the singleton is
// replaced by a call to a generated static helper that does nothing (returns the default) when the receiver is null and
// is the original access otherwise; a reference result is guarded the same way where it is dereferenced at once
// (`NCombatRoom.Instance.Ui.AddChildSafely(..)`). With a non-null receiver nothing changes, so game logic is unchanged.
//
// A singleton read that is not dereferenced directly (stored in a local or a field, passed as an argument) is not
// patched. Each one must be listed in escapes.txt (reviewed: null-checked before use, or off the headless path); the
// tool fails (exit 1) on an unlisted one, on a listed one that is gone, and on a dereference it cannot rewrite. That
// is what makes a new gap fail the referee build.
// Usage: dotnet run -- <sts2.dll> <GodotStubs output dir> <escapes.txt>   (patches in place)
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Mono.Cecil;
using Mono.Cecil.Cil;
using Mono.Cecil.Rocks;

#nullable enable

string[] gameplay =
[
    "Achievements", "CardSelection", "Combat", "Commands", "Context", "Daily", "Entities", "Events", "Extensions",
    "Factories", "GameActions", "Hooks", "Map", "Models", "MonsterMoves", "Odds", "Random", "Rewards", "Rooms", "Runs",
    "Unlocks", "ValueProps",
];

var dllPath = args[0];
var resolver = new DefaultAssemblyResolver();
foreach (var dir in resolver.GetSearchDirectories()) resolver.RemoveSearchDirectory(dir);
resolver.AddSearchDirectory(args[1]);
resolver.AddSearchDirectory(Path.GetDirectoryName(Path.GetFullPath(dllPath))!);
var module = ModuleDefinition.ReadModule(dllPath, new ReaderParameters { AssemblyResolver = resolver, ReadingMode = ReadingMode.Immediate });
var escapesListed = File.ReadAllLines(args[2]).Select(l => l.Split('#')[0].Trim()).Where(l => l.Length > 0).ToHashSet();

bool IsGodotNode(TypeReference? t) => DerivesFrom(t, "Godot.Node");
bool IsGodotObject(TypeReference? t) => DerivesFrom(t, "Godot.GodotObject");

bool DerivesFrom(TypeReference? t, string godotBase)
{
    try
    {
        for (var d = t?.Resolve(); d != null; d = d.BaseType?.Resolve())
        {
            if (d.FullName == godotBase) return true;
        }
    }
    catch (AssemblyResolutionException)
    {
    }
    return false;
}

static bool IsGameplay(TypeDefinition type, string[] gameplay)
{
    while (type.DeclaringType != null) type = type.DeclaringType;
    var parts = type.Namespace.Split('.');
    return parts.Length >= 4 && parts[0] == "MegaCrit" && parts[2] == "Core" && gameplay.Contains(parts[3]);
}

// A static `Instance` getter of a game node type: the singletons that are null headless.
bool IsSingleton(Instruction i) =>
    i.OpCode == OpCodes.Call && i.Operand is MethodReference { Name: "get_Instance", HasThis: false, Parameters.Count: 0 } m
    && m.DeclaringType.Scope == module && IsGodotNode(m.DeclaringType);

static (int Pop, int Push) StackEffect(Instruction i)
{
    if (i.Operand is MethodReference m && (i.OpCode == OpCodes.Call || i.OpCode == OpCodes.Callvirt || i.OpCode == OpCodes.Newobj))
    {
        var pop = m.Parameters.Count + (m.HasThis && i.OpCode != OpCodes.Newobj ? 1 : 0);
        var push = i.OpCode == OpCodes.Newobj || m.ReturnType.MetadataType != MetadataType.Void ? 1 : 0;
        return (pop, push);
    }
    // `ret` and `leave` end the walk (Consumer stops on them); counting them as consuming the value reports it as an escape.
    if (i.OpCode.Code is Code.Ret or Code.Leave or Code.Leave_S) return (int.MaxValue, 0);
    static int Count(StackBehaviour b) => b switch
    {
        StackBehaviour.Pop0 or StackBehaviour.Push0 => 0,
        StackBehaviour.Pop1 or StackBehaviour.Popi or StackBehaviour.Popref or StackBehaviour.Push1 or StackBehaviour.Pushi
            or StackBehaviour.Pushi8 or StackBehaviour.Pushr4 or StackBehaviour.Pushr8 or StackBehaviour.Pushref => 1,
        StackBehaviour.Pop1_pop1 or StackBehaviour.Popi_pop1 or StackBehaviour.Popi_popi or StackBehaviour.Popi_popi8
            or StackBehaviour.Popi_popr4 or StackBehaviour.Popi_popr8 or StackBehaviour.Popref_pop1 or StackBehaviour.Popref_popi
            or StackBehaviour.Push1_push1 => 2,
        StackBehaviour.Popi_popi_popi or StackBehaviour.Popref_popi_popi or StackBehaviour.Popref_popi_popi8
            or StackBehaviour.Popref_popi_popr4 or StackBehaviour.Popref_popi_popr8 or StackBehaviour.Popref_popi_popref => 3,
        _ => throw new InvalidOperationException($"stack behaviour {b}"),
    };
    return (Count(i.OpCode.StackBehaviourPop), Count(i.OpCode.StackBehaviourPush));
}

// The instruction that consumes the value `source` pushes, walking forward in straight-line code; null if the walk
// meets a branch or a branch target first (the value then flows through control flow and is reported as an escape).
Instruction? Consumer(MethodBody body, Instruction source, HashSet<Instruction> targets)
{
    var depth = 1;
    for (var i = source.Next; i != null; i = i.Next)
    {
        if (targets.Contains(i)) return null;
        var (pop, push) = StackEffect(i);
        if (pop >= depth) return i;
        if (i.OpCode.FlowControl is FlowControl.Branch or FlowControl.Cond_Branch or FlowControl.Return or FlowControl.Throw) return null;
        depth += push - pop;
    }
    return null;
}

// `this` of the consumer is the value when the consumer pops exactly down to it.
bool ConsumedAsReceiver(Instruction consumer, Instruction source, HashSet<Instruction> targets)
{
    var depth = 1;
    for (var i = source.Next; i != consumer; i = i!.Next)
    {
        var (pop, push) = StackEffect(i!);
        depth += push - pop;
    }
    var (p, _) = StackEffect(consumer);
    return p == depth && (consumer.OpCode == OpCodes.Ldfld
        || (consumer.Operand is MethodReference { HasThis: true } && (consumer.OpCode == OpCodes.Callvirt || consumer.OpCode == OpCodes.Call))
        || (consumer.OpCode == OpCodes.Call && consumer.Operand is MethodReference { HasThis: false } ext && IsExtension(ext)));
}

// `parent.AddChildSafely(child)`: an extension method called on the value, guarded like an instance member.
static bool IsExtension(MethodReference m)
{
    try
    {
        return m.Resolve()?.CustomAttributes.Any(a => a.AttributeType.FullName == "System.Runtime.CompilerServices.ExtensionAttribute") == true;
    }
    catch (AssemblyResolutionException)
    {
        return false;
    }
}

static bool IsNullCheck(Instruction c) =>
    c.OpCode.Code is Code.Brtrue or Code.Brtrue_S or Code.Brfalse or Code.Brfalse_S or Code.Isinst
    || (c.OpCode == OpCodes.Dup && c.Next.OpCode.Code is Code.Brtrue or Code.Brtrue_S or Code.Brfalse or Code.Brfalse_S)
    || (c.OpCode == OpCodes.Call && c.Operand is MethodReference { Name: "op_Equality" or "op_Inequality" or "IsInstanceValid" });

var guardType = new TypeDefinition("MegaCrit.Sts2.Core.Referee", "NullGuard",
    TypeAttributes.Public | TypeAttributes.Abstract | TypeAttributes.Sealed | TypeAttributes.Class, module.TypeSystem.Object);
var helpers = new Dictionary<string, MethodDefinition>();

// `static R Guard(T self, args...) => self == null ? default : self.M(args...)` for the member access `access`.
MethodDefinition Helper(Instruction access)
{
    var key = $"{access.OpCode} {access.Operand}";
    if (helpers.TryGetValue(key, out var existing)) return existing;
    TypeReference self, ret;
    var extension = false;
    var parameters = new List<TypeReference>();
    if (access.Operand is FieldReference f)
    {
        self = f.DeclaringType;
        ret = f.FieldType;
    }
    else
    {
        var m = (MethodReference)access.Operand;
        if (m is GenericInstanceMethod || m.DeclaringType.IsGenericInstance || m.ContainsGenericParameter)
            throw new InvalidOperationException($"generic member {m.FullName}");
        extension = !m.HasThis;
        self = extension ? m.Parameters[0].ParameterType : m.DeclaringType;
        ret = m.ReturnType;
        parameters.AddRange(m.Parameters.Skip(extension ? 1 : 0).Select(p => p.ParameterType));
    }
    // The helper lives in its own type, so the member must be visible outside the declaring type.
    var visible = access.Operand switch
    {
        FieldReference fr => fr.Resolve() is { IsPublic: true } or { IsAssembly: true },
        MethodReference mr => mr.Resolve() is { IsPublic: true } or { IsAssembly: true },
        _ => false,
    };
    if (!visible || self.IsValueType || ret.IsByReference || parameters.Any(p => p.IsByReference))
        throw new InvalidOperationException($"cannot guard {access}");
    var helper = new MethodDefinition($"Guard{helpers.Count}", MethodAttributes.Public | MethodAttributes.Static | MethodAttributes.HideBySig, ret);
    helper.Parameters.Add(new ParameterDefinition("self", ParameterAttributes.None, self));
    foreach (var p in parameters) helper.Parameters.Add(new ParameterDefinition(p));
    var il = helper.Body.GetILProcessor();
    var notNull = Instruction.Create(OpCodes.Ldarg_0);
    il.Emit(OpCodes.Ldarg_0);
    il.Emit(OpCodes.Brtrue, notNull);
    if (ret.MetadataType != MetadataType.Void)
    {
        var result = new VariableDefinition(ret);
        helper.Body.Variables.Add(result);
        helper.Body.InitLocals = true;
        il.Emit(OpCodes.Ldloc, result);
    }
    il.Emit(OpCodes.Ret);
    il.Append(notNull);
    for (var a = 1; a <= parameters.Count; a++) il.Emit(OpCodes.Ldarg, helper.Parameters[a]);
    if (access.Operand is FieldReference field) il.Emit(OpCodes.Ldfld, field);
    else if (extension) il.Emit(OpCodes.Call, (MethodReference)access.Operand);
    else il.Emit(OpCodes.Callvirt, (MethodReference)access.Operand);
    il.Emit(OpCodes.Ret);
    guardType.Methods.Add(helper);
    return helpers[key] = helper;
}

var errors = new List<string>();
var escapes = new HashSet<string>();
var patched = 0;
foreach (var type in module.GetTypes().Where(t => IsGameplay(t, gameplay)).ToList())
{
    foreach (var method in type.Methods.Where(m => m.HasBody))
    {
        var body = method.Body;
        var sources = body.Instructions.Where(IsSingleton).ToList();
        if (sources.Count == 0) continue;
        body.SimplifyMacros();
        var targets = body.Instructions.SelectMany(i => i.Operand switch
        {
            Instruction t => [t],
            Instruction[] ts => ts,
            _ => Array.Empty<Instruction>(),
        }).Concat(body.ExceptionHandlers.SelectMany(h => new[] { h.TryStart, h.HandlerStart, h.FilterStart }.OfType<Instruction>())).ToHashSet();
        var name = $"{type.FullName}::{method.Name}";
        var work = new Queue<(Instruction Source, string What)>(sources.Select(s => (s, ((MethodReference)s.Operand).DeclaringType.Name + ".Instance")));
        while (work.TryDequeue(out var item))
        {
            var consumer = Consumer(body, item.Source, targets);
            if (consumer != null && IsNullCheck(consumer)) continue;
            if (consumer == null || !ConsumedAsReceiver(consumer, item.Source, targets))
            {
                escapes.Add($"{name} {item.What}");
                continue;
            }
            var member = consumer.Operand is MethodReference mr ? mr.Name : ((FieldReference)consumer.Operand).Name;
            try
            {
                var helper = Helper(consumer);
                consumer.OpCode = OpCodes.Call;
                consumer.Operand = helper;
                patched++;
                // A node result (`.Ui`, `.GetCreatureNode(..)`) is null too and is followed; any other reference result
                // (a Task, a list) is an escape to review.
                if (IsGodotObject(helper.ReturnType)) work.Enqueue((consumer, $"{item.What}.{member}"));
                else if (!helper.ReturnType.IsValueType && helper.ReturnType.MetadataType != MetadataType.Void)
                    escapes.Add($"{name} {item.What}.{member}");
            }
            catch (InvalidOperationException e)
            {
                errors.Add($"{name}: {item.What}.{member}: {e.Message}");
            }
        }
        body.OptimizeMacros();
    }
}

// A reference result that is not dereferenced at once (stored, passed on) is an escape of its own only when it is the
// singleton itself; a guarded member result that escapes is reported too, under its chain name.
foreach (var e in escapes.Where(e => !escapesListed.Contains(e)).Order()) errors.Add($"{e} is read without a guard; review it and list it in escapes.txt");
foreach (var e in escapesListed.Where(e => !escapes.Contains(e)).Order()) errors.Add($"{e} is in escapes.txt but no longer read without a guard");
module.Types.Add(guardType);
foreach (var e in errors) Console.Error.WriteLine($"nullguard: {e}");
if (errors.Count > 0) return 1;
var outPath = dllPath + ".nullguard";
module.Write(outPath);
File.Delete(dllPath);
File.Move(outPath, dllPath);
Console.WriteLine($"  nullguard: {patched} member accesses on node singletons guarded with {helpers.Count} helpers");
return 0;
