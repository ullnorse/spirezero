// Checks sts2-cli's Godot stubs (GodotSharp.dll) against what the patched sts2.dll references. A game method whose body
// references a GodotSharp type or member the stubs lack fails to JIT, and calling it throws MissingMethodException as a
// whole, even when the reference sits in a branch the headless run never takes. PhantasmalGardener's Enlarge
// (Mathf.Log) and RollingBoulderPower (GodotObject.Connect) were lost this way. A member can also be referenced through a
// game type that inherits it from a Godot base (`someNode.Foo()` emitted as `NSomeNode::Foo`): the reference names a
// game type, so it is checked by resolving it up the base chain.
//
// Fails (exit 1) when such a method is in a game-logic namespace (Gameplay below) and not in allowed.txt, or when any
// method references a missing Godot.Mathf member. Also fails on an allowed.txt entry that no longer needs to be there.
// Usage: dotnet run -- <sts2.dll> <GodotStubs output dir> <allowed.txt>
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Mono.Cecil;

#nullable enable

string[] gameplay =
[
    "Achievements", "CardSelection", "Combat", "Commands", "Context", "Daily", "Entities", "Events", "Extensions",
    "Factories", "GameActions", "Hooks", "Map", "Models", "MonsterMoves", "Odds", "Random", "Rewards", "Rooms", "Runs",
    "Unlocks", "ValueProps",
];

var resolver = new DefaultAssemblyResolver();
foreach (var dir in resolver.GetSearchDirectories()) resolver.RemoveSearchDirectory(dir);
resolver.AddSearchDirectory(args[1]);
resolver.AddSearchDirectory(Path.GetDirectoryName(Path.GetFullPath(args[0]))!);
var module = ModuleDefinition.ReadModule(args[0], new ReaderParameters { AssemblyResolver = resolver });
var allowed = File.ReadAllLines(args[2]).Select(l => l.Split('#')[0].Trim()).Where(l => l.Length > 0).ToHashSet();

static bool IsGodot(TypeReference t)
{
    while (t is TypeSpecification spec) t = spec.ElementType;
    if (t is GenericParameter) return false;
    while (t.DeclaringType != null) t = t.DeclaringType;
    return t.Scope is AssemblyNameReference { Name: "GodotSharp" };
}

var resolved = new Dictionary<string, bool>();
bool Unresolved(MemberReference r, Func<object?> resolve) =>
    resolved.TryGetValue(r.FullName, out var missing) ? missing : resolved[r.FullName] = resolve() == null;

// A member reference (not a definition) whose declaring type is defined in sts2.dll names a member inherited from a base
// type. It is missing when it does not resolve and some base is a Godot type (a non-Godot base, such as a BCL type, is
// outside the resolver's search path and is not checked).
bool InheritedFromGodot(MemberReference r, Func<object?> resolve)
{
    TypeReference t = r.DeclaringType;
    while (t is TypeSpecification spec) t = spec.ElementType;
    if (t is not TypeDefinition def || def.Module != module) return false;
    for (var b = def.BaseType; b != null;)
    {
        if (IsGodot(b))
        {
            try { return Unresolved(r, resolve); }
            catch (AssemblyResolutionException) { return false; }
        }
        try { b = b.Resolve()?.BaseType; }
        catch (AssemblyResolutionException) { return false; }
    }
    return false;
}

bool Missing(object? operand) => operand switch
{
    GenericInstanceType git => Missing(git.ElementType) || git.GenericArguments.Any(Missing),
    TypeSpecification spec => Missing(spec.ElementType),
    GenericParameter => false,
    TypeReference t => IsGodot(t) && Unresolved(t, t.Resolve),
    MethodReference m => (m is GenericInstanceMethod gim && gim.GenericArguments.Any(Missing)) || Missing(m.DeclaringType)
        || (IsGodot(m.DeclaringType) && Unresolved(m, m.Resolve)) || (m is not MethodDefinition && InheritedFromGodot(m, m.Resolve)),
    FieldReference f => Missing(f.DeclaringType) || (IsGodot(f.DeclaringType) && Unresolved(f, f.Resolve))
        || (f is not FieldDefinition && InheritedFromGodot(f, f.Resolve)),
    _ => false,
};

var errors = new List<string>();
var failing = new HashSet<string>();
foreach (var type in module.GetTypes())
{
    var ns = type;
    while (ns.DeclaringType != null) ns = ns.DeclaringType;
    var parts = ns.Namespace.Split('.');
    var isGameplay = parts.Length >= 4 && parts[0] == "MegaCrit" && parts[2] == "Core" && gameplay.Contains(parts[3]);
    foreach (var method in type.Methods.Where(m => m.HasBody))
    {
        var refs = method.Body.Instructions.Select(i => i.Operand).Where(Missing).Cast<MemberReference>()
            .Concat(method.Body.Variables.Select(v => v.VariableType).Where(Missing)).Select(r => r.FullName).Distinct().ToList();
        if (refs.Count == 0) continue;
        var name = $"{type.FullName}::{method.Name}";
        failing.Add(name);
        var mathf = refs.Where(r => r.Contains("Godot.Mathf::")).ToList();
        if (mathf.Count > 0) errors.Add($"{name} references missing {string.Join(", ", mathf)}");
        else if (isGameplay && !allowed.Contains(name)) errors.Add($"{name} references missing {string.Join(", ", refs)}");
    }
}
errors.AddRange(allowed.Where(a => !failing.Contains(a)).Select(a => $"{a} is in allowed.txt but no longer references a missing member"));
foreach (var e in errors) Console.Error.WriteLine($"stubcheck: {e}");
return errors.Count == 0 ? 0 : 1;
