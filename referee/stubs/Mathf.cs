// Replaces sts2-cli's Godot.Mathf stub (removed from GodotStubs/Math.cs by referee/setup.sh). It has every Mathf member
// sts2.dll references (`referee/stubcheck` fails the build on a missing one) plus the ones sts2-cli's stub had, each with
// the semantics of the shipped GodotSharp 4.5: float overloads compute in float (MathF), double ones in double, Round is
// MidpointRounding.ToEven, Sign returns int, IsEqualApprox is relative. Overload set and signatures match GodotSharp, so
// a call site binds to the same member it binds to in the real game.
namespace Godot;

public static class Mathf
{
    public const float Tau = (float)Math.PI * 2f;
    public const float Pi = (float)Math.PI;
    public const float Inf = float.PositiveInfinity;
    public const float NaN = float.NaN;
    public const float Epsilon = 1e-06f;

    public static int Abs(int s) => Math.Abs(s);
    public static float Abs(float s) => Math.Abs(s);
    public static double Abs(double s) => Math.Abs(s);

    public static float AngleDifference(float from, float to)
    {
        float difference = (to - from) % Tau;
        return 2f * difference % Tau - difference;
    }

    public static float Atan2(float y, float x) => MathF.Atan2(y, x);
    public static float Ceil(float s) => MathF.Ceiling(s);
    public static int CeilToInt(float s) => (int)MathF.Ceiling(s);
    public static int CeilToInt(double s) => (int)Math.Ceiling(s);
    public static int Clamp(int value, int min, int max) => Math.Clamp(value, min, max);
    public static float Clamp(float value, float min, float max) => Math.Clamp(value, min, max);
    public static double Clamp(double value, double min, double max) => Math.Clamp(value, min, max);
    public static float Cos(float s) => MathF.Cos(s);
    public static float DegToRad(float deg) => deg * (Pi / 180f);
    public static double DegToRad(double deg) => deg * (Math.PI / 180.0);
    public static float Floor(float s) => MathF.Floor(s);
    public static int FloorToInt(float s) => (int)MathF.Floor(s);
    public static int FloorToInt(double s) => (int)Math.Floor(s);
    public static float InverseLerp(float from, float to, float weight) => (weight - from) / (to - from);
    public static double InverseLerp(double from, double to, double weight) => (weight - from) / (to - from);

    public static bool IsEqualApprox(float a, float b)
    {
        if (a == b)
        {
            return true;
        }
        float tolerance = Epsilon * Math.Abs(a);
        if (tolerance < Epsilon)
        {
            tolerance = Epsilon;
        }
        return Math.Abs(a - b) < tolerance;
    }

    public static bool IsZeroApprox(float s) => Math.Abs(s) < Epsilon;
    public static float Lerp(float from, float to, float weight) => from + (to - from) * weight;
    public static double Lerp(double from, double to, double weight) => from + (to - from) * weight;
    public static float LerpAngle(float from, float to, float weight) => from + AngleDifference(from, to) * weight;
    // 20 / ln(10) as GodotSharp rounds it to float.
    public static float LinearToDb(float linear) => MathF.Log(linear) * 8.685889f;
    public static float Log(float s) => MathF.Log(s);
    public static double Log(double s) => Math.Log(s);
    public static int Max(int a, int b) => Math.Max(a, b);
    public static float Max(float a, float b) => Math.Max(a, b);
    public static double Max(double a, double b) => Math.Max(a, b);
    public static int Min(int a, int b) => Math.Min(a, b);
    public static float Min(float a, float b) => Math.Min(a, b);
    public static double Min(double a, double b) => Math.Min(a, b);
    public static float Pow(float x, float y) => MathF.Pow(x, y);
    public static float RadToDeg(float rad) => rad * 57.29578f;
    public static float Remap(float value, float inFrom, float inTo, float outFrom, float outTo) =>
        Lerp(outFrom, outTo, InverseLerp(inFrom, inTo, value));
    public static float Round(float s) => MathF.Round(s);
    public static int RoundToInt(float s) => (int)MathF.Round(s);
    public static int RoundToInt(double s) => (int)Math.Round(s);
    public static int Sign(int s) => Math.Sign(s);
    public static int Sign(float s) => Math.Sign(s);
    public static int Sign(double s) => Math.Sign(s);
    public static float Sin(float s) => MathF.Sin(s);
    public static double Sin(double s) => Math.Sin(s);
    public static float Snapped(float s, float step) => step != 0f ? MathF.Floor(s / step + 0.5f) * step : s;
    public static float Sqrt(float s) => MathF.Sqrt(s);
    public static float Tan(float s) => MathF.Tan(s);

    public static float Wrap(float value, float min, float max)
    {
        float range = max - min;
        if (IsZeroApprox(range))
        {
            return min;
        }
        return min + ((value - min) % range + range) % range;
    }
}
