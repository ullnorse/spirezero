//! A short list stored inline, with a spill for the rare long one (`docs/specs/stage4-design.md`
//! section 10.2), and the capacities of the state's inline lists.
//!
//! A search copies the state for every candidate action, so a list that lives inside its owner
//! copies with the owner's own memcpy instead of a heap block of its own. The one test a read
//! needs, `len <= CAP`, is also the slice's bounds check: reads cost what a `Vec` read costs. A
//! list longer than `CAP` moves to a `Vec` (the spill), which changes where the items live and
//! nothing else: equality, hashing and `Debug` see the items only.

use std::fmt;
use std::hash::{Hash, Hasher};

/// The capacities of the state's inline lists, in one place. The `tiny-caps` feature makes each
/// of them 1, so that every list spills from its second item (the forced-spill gate).
pub mod caps {
    /// The powers on one creature (`Creature::powers`): over 8 in 9 of a million normal states.
    pub const POWERS: usize = if cfg!(feature = "tiny-caps") { 1 } else { 8 };
    /// The player's relics (`Player::relics`): over 32 in no normal state (37 at most in the
    /// community's runs of the current build).
    pub const RELICS: usize = if cfg!(feature = "tiny-caps") { 1 } else { 32 };
}

/// Up to `CAP` items inline, more in `spill`.
///
/// Invariant: `len <= CAP` means the items are `items[..len]` (`spill` may hold stale items,
/// kept for its buffer); `len > CAP` means they are `spill`, which then has `len` items (the
/// inline array is stale).
#[derive(Clone)]
pub struct InlineList<T: Copy + Default, const CAP: usize> {
    len: usize,
    items: [T; CAP],
    spill: Vec<T>,
}

impl<T: Copy + Default, const CAP: usize> Default for InlineList<T, CAP> {
    #[inline]
    fn default() -> Self {
        Self {
            len: 0,
            items: [T::default(); CAP],
            spill: Vec::new(),
        }
    }
}

impl<T: Copy + Default, const CAP: usize> InlineList<T, CAP> {
    /// An empty list.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of items.
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the list is empty.
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The items, in order.
    #[must_use]
    #[expect(
        clippy::inline_always,
        reason = "every power read goes through it; the branch must fold into the caller's loop"
    )]
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        let len = self.len();
        if len <= CAP {
            &self.items[..len]
        } else {
            // Two loads, no call: a call here would make every reader save registers.
            &self.spill
        }
    }

    /// The items, in order, mutably.
    #[must_use]
    #[expect(
        clippy::inline_always,
        reason = "every power read goes through it; the branch must fold into the caller's loop"
    )]
    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let len = self.len();
        if len <= CAP {
            &mut self.items[..len]
        } else {
            &mut self.spill
        }
    }

    /// Adds `item` at the end.
    #[inline]
    pub fn push(&mut self, item: T) {
        let len = self.len();
        if len < CAP {
            self.items[len] = item;
            self.len += 1;
        } else {
            self.push_spill(item);
        }
    }

    /// [`InlineList::push`] into a full inline array or a spilled list.
    #[cold]
    #[inline(never)]
    fn push_spill(&mut self, item: T) {
        if self.len() == CAP {
            self.spill.clear();
            self.spill.extend_from_slice(&self.items);
        }
        self.spill.push(item);
        self.len += 1;
    }

    /// Adds `items` at the end, in order.
    pub fn extend_from_slice(&mut self, items: &[T]) {
        for &item in items {
            self.push(item);
        }
    }

    /// Removes every item (the spill keeps its buffer).
    #[inline]
    pub const fn clear(&mut self) {
        self.len = 0;
    }

    /// Removes and returns the item at `index`, shifting the later ones down.
    ///
    /// # Panics
    /// If `index` is out of bounds.
    #[inline]
    pub fn remove(&mut self, index: usize) -> T {
        let len = self.len();
        if len <= CAP {
            let item = self.items[..len][index];
            self.items.copy_within(index + 1..len, index);
            self.len -= 1;
            item
        } else {
            self.remove_spilled(index)
        }
    }

    #[cold]
    #[inline(never)]
    fn remove_spilled(&mut self, index: usize) -> T {
        let item = self.spill.remove(index);
        self.len -= 1;
        self.unspill();
        item
    }

    /// Keeps the items for which `keep` holds, in order.
    #[inline]
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let len = self.len();
        if len <= CAP {
            let mut kept = 0;
            for i in 0..len {
                let item = self.items[i];
                if keep(&item) {
                    self.items[kept] = item;
                    kept += 1;
                }
            }
            self.len = kept;
        } else {
            self.spill.retain(keep);
            self.len = self.spill.len();
            self.unspill();
        }
    }

    /// Moves a spilled list that fits again back inline.
    fn unspill(&mut self) {
        let len = self.len();
        if len <= CAP {
            self.items[..len].copy_from_slice(&self.spill[..len]);
        }
    }

    /// The items, in order.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    /// The items, in order, mutably.
    #[inline]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }
}

impl<T: Copy + Default, const CAP: usize> std::ops::Deref for InlineList<T, CAP> {
    type Target = [T];

    #[expect(
        clippy::inline_always,
        reason = "every power read goes through it; the branch must fold into the caller's loop"
    )]
    #[inline(always)]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Copy + Default, const CAP: usize> std::ops::DerefMut for InlineList<T, CAP> {
    #[expect(
        clippy::inline_always,
        reason = "every power read goes through it; the branch must fold into the caller's loop"
    )]
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<'a, T: Copy + Default, const CAP: usize> IntoIterator for &'a InlineList<T, CAP> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: Copy + Default, const CAP: usize> IntoIterator for &'a mut InlineList<T, CAP> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// The inline items [`copy_list_from`] always copies: a constant-size copy this short is a few
/// register moves, where the whole array would be a `memcpy` call.
const COPY_FIRST: usize = 4;

/// The largest rest of an inline array (in bytes) that [`copy_list_from`] copies whole rather
/// than up to the length.
const SHORT_TAIL: usize = 128;

/// A copy that reuses the spill's buffer. The first [`COPY_FIRST`] inline slots are copied
/// whatever the length (a fixed-size copy, inlined), the rest of the array only when the source
/// uses it, and the spill only when the source has spilled. Stale slots are never read.
#[inline]
pub(crate) fn copy_list_from<T: Copy + Default, const CAP: usize>(
    dst: &mut InlineList<T, CAP>,
    src: &InlineList<T, CAP>,
) {
    let first = COPY_FIRST.min(CAP);
    dst.len = src.len;
    dst.items[..first].copy_from_slice(&src.items[..first]);
    if src.len() > first {
        copy_rest(dst, src, first);
    }
}

/// [`copy_list_from`] for a source longer than its first slots.
#[inline(never)]
fn copy_rest<T: Copy + Default, const CAP: usize>(
    dst: &mut InlineList<T, CAP>,
    src: &InlineList<T, CAP>,
    first: usize,
) {
    let len = src.len();
    if len > CAP {
        crate::copy_vec_from(&mut dst.spill, &src.spill);
    } else if (CAP - first) * size_of::<T>() <= SHORT_TAIL {
        // A fixed-size copy this short is register moves, cheaper than a sized `memcpy` call.
        dst.items[first..].copy_from_slice(&src.items[first..]);
    } else {
        dst.items[first..len].copy_from_slice(&src.items[first..len]);
    }
}

impl<T: Copy + Default + PartialEq, const CAP: usize> PartialEq for InlineList<T, CAP> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Copy + Default + Eq, const CAP: usize> Eq for InlineList<T, CAP> {}

impl<T: Copy + Default + PartialEq, const CAP: usize, const N: usize> PartialEq<[T; N]>
    for InlineList<T, CAP>
{
    fn eq(&self, other: &[T; N]) -> bool {
        self.as_slice() == other
    }
}

impl<T: Copy + Default + PartialEq, const CAP: usize> PartialEq<Vec<T>> for InlineList<T, CAP> {
    fn eq(&self, other: &Vec<T>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

/// As a `Vec`'s: the length, then the items.
impl<T: Copy + Default + Hash, const CAP: usize> Hash for InlineList<T, CAP> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

/// As a `Vec`'s: the items.
impl<T: Copy + Default + fmt::Debug, const CAP: usize> fmt::Debug for InlineList<T, CAP> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<T: Copy + Default, const CAP: usize> FromIterator<T> for InlineList<T, CAP> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut list = Self::new();
        for item in iter {
            list.push(item);
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::InlineList;

    /// Every operation at a capacity that forces spills agrees with a `Vec`.
    #[test]
    fn spilled_list_matches_vec() {
        let mut list: InlineList<u32, 2> = InlineList::new();
        let mut vec: Vec<u32> = Vec::new();
        for i in 0..6 {
            list.push(i);
            vec.push(i);
            assert_eq!(list, vec);
        }
        assert_eq!(list.remove(1), vec.remove(1));
        assert_eq!(list, vec);
        list.retain(|&x| x % 2 == 0);
        vec.retain(|&x| x % 2 == 0);
        assert_eq!(list, vec);
        assert_eq!(list.len(), 3);
        assert_eq!(list.remove(0), vec.remove(0));
        assert_eq!(list, vec);
        list.extend_from_slice(&[7, 8, 9]);
        vec.extend_from_slice(&[7, 8, 9]);
        assert_eq!(list, vec);
        let mut copy: InlineList<u32, 2> = InlineList::new();
        super::copy_list_from(&mut copy, &list);
        assert_eq!(copy, list);
        list.clear();
        assert!(list.is_empty());
        super::copy_list_from(&mut copy, &list);
        assert_eq!(copy, list);
        assert_eq!(format!("{copy:?}"), "[]");
    }
}
