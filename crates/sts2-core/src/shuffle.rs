//! The game's list shuffles (`ListExtensions.UnstableShuffle` / `StableShuffle`) and the .NET
//! `List<T>.Sort()` introsort that `StableShuffle` relies on for the order of ties.

use std::cmp::Ordering;

use smallvec::SmallVec;

use crate::rng::Rng;

/// Lists up to this length are finished with insertion sort.
const INTROSORT_SIZE_THRESHOLD: usize = 16;

/// `UnstableShuffle`: Fisher-Yates from the end, `j = rng.next_int(n + 1)`.
/// Draws `len - 1` values (none for `len <= 1`); the result depends on the input order.
///
/// # Panics
/// If `list` has more than `i32::MAX` elements.
pub fn unstable_shuffle<T>(list: &mut [T], rng: &mut Rng) {
    for n in (1..list.len()).rev() {
        let bound = i32::try_from(n + 1).expect("unstable_shuffle: list longer than i32::MAX");
        let j = usize::try_from(rng.next_int(bound)).expect("next_int is non-negative");
        list.swap(n, j);
    }
}

/// `StableShuffle` with the element's natural order: sort, then [`unstable_shuffle`].
pub fn stable_shuffle<T: Ord>(list: &mut [T], rng: &mut Rng) {
    stable_shuffle_by(list, rng, T::cmp);
}

/// `StableShuffle` with an explicit `CompareTo`: [`introsort_by`], then [`unstable_shuffle`].
/// Equal elements keep the order the .NET sort leaves them in.
pub fn stable_shuffle_by<T>(list: &mut [T], rng: &mut Rng, cmp: impl FnMut(&T, &T) -> Ordering) {
    introsort_by(list, cmp);
    unstable_shuffle(list, rng);
}

/// [`stable_shuffle_by`] comparing `key(a)` with `key(b)`, each element's key read once: the
/// sort sees the same comparison results, so the order is the same.
pub fn stable_shuffle_by_key<T: Copy, K: Ord + Copy>(
    list: &mut [T],
    rng: &mut Rng,
    key: impl Fn(&T) -> K,
) {
    let mut keyed: SmallVec<[(K, T); 32]> = list.iter().map(|x| (key(x), *x)).collect();
    introsort_by(&mut keyed, |a, b| a.0.cmp(&b.0));
    for (slot, &(_, x)) in list.iter_mut().zip(&keyed) {
        *slot = x;
    }
    unstable_shuffle(list, rng);
}

/// .NET's `ArraySortHelper` introsort, ported so that the order of equal elements matches
/// `List<T>.Sort()` exactly. The sort is not stable for lists longer than 16.
pub fn introsort_by<T>(list: &mut [T], mut cmp: impl FnMut(&T, &T) -> Ordering) {
    if list.len() < 2 {
        return;
    }
    let depth_limit = 2 * (list.len().ilog2() + 1);
    introsort(list, depth_limit, &mut cmp);
}

fn introsort<T, F: FnMut(&T, &T) -> Ordering>(
    mut keys: &mut [T],
    mut depth_limit: u32,
    cmp: &mut F,
) {
    while keys.len() > 1 {
        if keys.len() <= INTROSORT_SIZE_THRESHOLD {
            match keys.len() {
                2 => swap_if_greater(keys, 0, 1, cmp),
                3 => {
                    swap_if_greater(keys, 0, 1, cmp);
                    swap_if_greater(keys, 0, 2, cmp);
                    swap_if_greater(keys, 1, 2, cmp);
                }
                _ => insertion_sort(keys, cmp),
            }
            return;
        }
        if depth_limit == 0 {
            heap_sort(keys, cmp);
            return;
        }
        depth_limit -= 1;
        let pivot = pick_pivot_and_partition(keys, cmp);
        let (left, right) = keys.split_at_mut(pivot);
        introsort(&mut right[1..], depth_limit, cmp);
        keys = left;
    }
}

fn swap_if_greater<T, F: FnMut(&T, &T) -> Ordering>(
    keys: &mut [T],
    i: usize,
    j: usize,
    cmp: &mut F,
) {
    if i != j && cmp(&keys[i], &keys[j]).is_gt() {
        keys.swap(i, j);
    }
}

/// Median-of-three pivot moved to `hi - 1`, then a Hoare-style partition. Returns the pivot's
/// final index.
fn pick_pivot_and_partition<T, F: FnMut(&T, &T) -> Ordering>(keys: &mut [T], cmp: &mut F) -> usize {
    let hi = keys.len() - 1;
    let mid = hi >> 1;
    swap_if_greater(keys, 0, mid, cmp);
    swap_if_greater(keys, 0, hi, cmp);
    swap_if_greater(keys, mid, hi, cmp);
    let pivot = hi - 1;
    keys.swap(mid, pivot);
    let (mut left, mut right) = (0, pivot);
    // The pivot stays at `hi - 1` and `keys[0] <= pivot`, so both scans stop in bounds.
    while left < right {
        left += 1;
        while cmp(&keys[left], &keys[pivot]).is_lt() {
            left += 1;
        }
        right -= 1;
        while cmp(&keys[pivot], &keys[right]).is_lt() {
            right -= 1;
        }
        if left >= right {
            break;
        }
        keys.swap(left, right);
    }
    if left != pivot {
        keys.swap(left, pivot);
    }
    left
}

/// Stable insertion sort: each element moves left past strictly greater elements only.
fn insertion_sort<T, F: FnMut(&T, &T) -> Ordering>(keys: &mut [T], cmp: &mut F) {
    for i in 1..keys.len() {
        let mut j = i;
        while j > 0 && cmp(&keys[i], &keys[j - 1]).is_lt() {
            j -= 1;
        }
        keys[j..=i].rotate_right(1);
    }
}

fn heap_sort<T, F: FnMut(&T, &T) -> Ordering>(keys: &mut [T], cmp: &mut F) {
    let n = keys.len();
    for i in (1..=n >> 1).rev() {
        down_heap(keys, i, n, cmp);
    }
    for i in (2..=n).rev() {
        keys.swap(0, i - 1);
        down_heap(keys, 1, i - 1, cmp);
    }
}

/// Sifts the 1-based node `i` down within the first `n` elements.
fn down_heap<T, F: FnMut(&T, &T) -> Ordering>(keys: &mut [T], mut i: usize, n: usize, cmp: &mut F) {
    while i <= n >> 1 {
        let mut child = 2 * i;
        if child < n && cmp(&keys[child - 1], &keys[child]).is_lt() {
            child += 1;
        }
        if !cmp(&keys[i - 1], &keys[child - 1]).is_lt() {
            break;
        }
        keys.swap(i - 1, child - 1);
        i = child;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyed_shuffle_matches_the_comparator_shuffle() {
        // Lists past the insertion-sort threshold and past the inline buffer, with many ties.
        for len in [0_u32, 1, 2, 3, 5, 10, 16, 17, 31, 40, 70] {
            for seed in 0..20_u32 {
                let list: Vec<(u32, usize)> = (0..len)
                    .map(|i| (i.wrapping_mul(2_654_435_761) ^ seed) % 7)
                    .zip(0_usize..)
                    .collect();
                let mut a = list.clone();
                let mut b = list;
                stable_shuffle_by(&mut a, &mut Rng::new(u64::from(seed)), |x, y| x.0.cmp(&y.0));
                stable_shuffle_by_key(&mut b, &mut Rng::new(u64::from(seed)), |x| x.0);
                assert_eq!(a, b, "len {len} seed {seed}");
            }
        }
    }
}
