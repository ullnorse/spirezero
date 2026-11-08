namespace Golden;

/// <summary>Test element: ordered by key only, so equal keys expose the sort's tie order via the tag.</summary>
internal sealed class Tied : IComparable<Tied>
{
    public readonly int Key;
    public readonly int Tag;

    public Tied(int key, int tag)
    {
        Key = key;
        Tag = tag;
    }

    public int CompareTo(Tied other) => Key.CompareTo(other.Key);
}

/// <summary>
/// Port of the introsort described in docs/specs/rng-decimal.md §3.2 (from memory of dotnet/runtime
/// ArraySortHelper), used only to confirm that List&lt;T&gt;.Sort() orders ties exactly as the spec says.
/// </summary>
internal static class IntroSortCheck
{
    /// <summary>Returns 0 when the spec's introsort over <paramref name="input"/> equals <paramref name="actual"/> by tag.</summary>
    public static int Verify(Tied[] input, List<Tied> actual)
    {
        var copy = (Tied[])input.Clone();
        Sort(copy);
        for (int i = 0; i < copy.Length; i++)
        {
            if (copy[i].Tag != actual[i].Tag) return 1;
        }
        return 0;
    }

    private static int Cmp(Tied a, Tied b) => a.CompareTo(b);

    private static void Sort(Tied[] keys)
    {
        if (keys.Length < 2) return;
        IntroSort(keys.AsSpan(), 2 * (System.Numerics.BitOperations.Log2((uint)keys.Length) + 1));
    }

    private static void Swap(Span<Tied> k, int i, int j) => (k[i], k[j]) = (k[j], k[i]);

    private static void SwapIfGreater(Span<Tied> k, int i, int j)
    {
        if (i != j && Cmp(k[i], k[j]) > 0) Swap(k, i, j);
    }

    private static void IntroSort(Span<Tied> keys, int depthLimit)
    {
        while (keys.Length > 1)
        {
            if (keys.Length <= 16)
            {
                if (keys.Length == 2)
                {
                    SwapIfGreater(keys, 0, 1);
                    return;
                }
                if (keys.Length == 3)
                {
                    SwapIfGreater(keys, 0, 1);
                    SwapIfGreater(keys, 0, 2);
                    SwapIfGreater(keys, 1, 2);
                    return;
                }
                InsertionSort(keys);
                return;
            }
            if (depthLimit == 0)
            {
                HeapSort(keys);
                return;
            }
            depthLimit--;
            int p = PickPivotAndPartition(keys);
            IntroSort(keys.Slice(p + 1), depthLimit);
            keys = keys.Slice(0, p);
        }
    }

    private static int PickPivotAndPartition(Span<Tied> k)
    {
        int hi = k.Length - 1;
        int mid = hi >> 1;
        SwapIfGreater(k, 0, mid);
        SwapIfGreater(k, 0, hi);
        SwapIfGreater(k, mid, hi);
        Tied pivot = k[mid];
        Swap(k, mid, hi - 1);
        int left = 0, right = hi - 1;
        while (left < right)
        {
            while (Cmp(k[++left], pivot) < 0) { }
            while (Cmp(pivot, k[--right]) < 0) { }
            if (left >= right) break;
            Swap(k, left, right);
        }
        if (left != hi - 1) Swap(k, left, hi - 1);
        return left;
    }

    private static void InsertionSort(Span<Tied> k)
    {
        for (int i = 0; i < k.Length - 1; i++)
        {
            Tied t = k[i + 1];
            int j = i;
            while (j >= 0 && Cmp(t, k[j]) < 0)
            {
                k[j + 1] = k[j];
                j--;
            }
            k[j + 1] = t;
        }
    }

    private static void HeapSort(Span<Tied> k)
    {
        int n = k.Length;
        for (int i = n >> 1; i >= 1; i--) DownHeap(k, i, n);
        for (int i = n; i > 1; i--)
        {
            Swap(k, 0, i - 1);
            DownHeap(k, 1, i - 1);
        }
    }

    private static void DownHeap(Span<Tied> k, int i, int n)
    {
        Tied d = k[i - 1];
        while (i <= n >> 1)
        {
            int child = 2 * i;
            if (child < n && Cmp(k[child - 1], k[child]) < 0) child++;
            if (!(Cmp(d, k[child - 1]) < 0)) break;
            k[i - 1] = k[child - 1];
            i = child;
        }
        k[i - 1] = d;
    }
}
