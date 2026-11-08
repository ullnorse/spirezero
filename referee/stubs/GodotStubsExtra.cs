// GodotSharp members sts2-cli's stubs lack that game logic references (referee/setup.sh makes the stub types partial).
// A method referencing a missing member fails to JIT and throws MissingMethodException as a whole, even when the
// reference sits in a branch the headless run never takes (TestMode, a null node). `referee/stubcheck` lists them.
namespace Godot;

public partial class GodotObject
{
    // RollingBoulderPower.AfterPlayerTurnStart (only its TestMode.IsOff branch calls them).
    public Error Connect(StringName signal, Callable callable, uint flags = 0) => Error.Ok;
    public Variant CallDeferred(StringName method, params Variant[] args) => default;
}

public partial class CanvasItem
{
    // Crusher/Rocket.AfterAddedToRoom (KaiserCrabBoss) and TorchHeadAmalgam.OnDieToDoom, on a node that is null headless.
    public void SetVisible(bool visible) => Visible = visible;
}

public partial struct Vector2I
{
    // CrystalSphereGold.Size, which places the item on the Crystal Sphere grid.
    public static Vector2I One { get; } = new(1, 1);
}

public partial struct Callable
{
    // GameAction.Cancel: Callable.From(() => _completionSource.TrySetCanceled()).CallDeferred(). Headless there is no
    // idle frame, so CallDeferred runs it at once, as it does for an Action.
    public static Callable From<TResult>(Func<TResult> func) => new(func);
    public static Callable From<T1, T2, T3>(Action<T1, T2, T3> action) => new(action);
    public static Callable From<T1, T2, T3, T4>(Action<T1, T2, T3, T4> action) => new(action);

    private void InvokeParameterless()
    {
        switch (_delegate)
        {
            case Action action:
                action();
                break;
            case Delegate d when d.Method.GetParameters().Length == 0:
                d.DynamicInvoke();
                break;
        }
    }
}

public partial class SceneTreeTimer
{
    // CombatState.GodotTimerTask (GetCreatureAsync's timeout, whose TestMode branch uses Task.Delay) and Cmd.WaitInternal.
    public new class SignalName : GodotObject.SignalName
    {
        public static readonly StringName Timeout = "timeout";
    }
}
