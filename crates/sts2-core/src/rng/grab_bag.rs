//! `GrabBag<T>` (`Helpers/GrabBag.cs`): weighted picks without replacement, in `f64`.

use super::Rng;

/// A weighted bag. Entries keep insertion order; removal shifts later entries down.
#[derive(Clone, Debug, PartialEq)]
pub struct GrabBag<T> {
    entries: Vec<(T, f64)>,
    total_weight: f64,
}

impl<T> Default for GrabBag<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            total_weight: 0.0,
        }
    }
}

impl<T> GrabBag<T> {
    /// An empty bag.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `Add(element, weight)`.
    pub fn add(&mut self, element: T, weight: f64) {
        self.entries.push((element, weight));
        self.total_weight += weight;
    }

    /// `Any()`.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `GrabAndRemove(rng, predicate)`: `None` (with no draw) if no entry satisfies the
    /// predicate; otherwise redraws until the drawn entry satisfies it, and removes it.
    pub fn grab_and_remove(&mut self, rng: &mut Rng, predicate: impl Fn(&T) -> bool) -> Option<T> {
        if !self.entries.iter().any(|(e, _)| predicate(e)) {
            return None;
        }
        loop {
            let index = self.grab_index(rng)?;
            if predicate(&self.entries[index].0) {
                let (element, weight) = self.entries.remove(index);
                self.total_weight -= weight;
                return Some(element);
            }
        }
    }

    /// `GrabIndex(rng)`: `NextDouble() * totalWeight`, then the first entry whose cumulative
    /// weight exceeds it.
    fn grab_index(&self, rng: &mut Rng) -> Option<usize> {
        let roll = rng.next_double() * self.total_weight;
        let mut cumulative = 0.0;
        self.entries.iter().position(|(_, weight)| {
            cumulative += weight;
            roll < cumulative
        })
    }
}
