//! The table behind every generated `Kind::from_id`.

/// Ids to kinds: FNV-1a of the id, linear probing in a power-of-two table at most a quarter
/// full, so a lookup is one hash and usually one string comparison.
#[derive(Debug)]
pub struct IdTable<K> {
    slots: Vec<Option<(&'static str, K)>>,
    mask: usize,
}

impl<K: Copy> IdTable<K> {
    /// The table of `entries`, whose ids are unique.
    pub fn new(entries: impl ExactSizeIterator<Item = (&'static str, K)>) -> Self {
        let size = (entries.len() * 4).next_power_of_two();
        let mut table = Self {
            slots: vec![None; size],
            mask: size - 1,
        };
        for (id, kind) in entries {
            let mut i = table.slot(id);
            while let Some((other, _)) = table.slots[i] {
                debug_assert_ne!(other, id, "ids are unique");
                i = (i + 1) & table.mask;
            }
            table.slots[i] = Some((id, kind));
        }
        table
    }

    /// The kind with id `id`.
    pub fn get(&self, id: &str) -> Option<K> {
        let mut i = self.slot(id);
        loop {
            match self.slots[i] {
                None => return None,
                Some((entry, kind)) if entry == id => return Some(kind),
                Some(_) => i = (i + 1) & self.mask,
            }
        }
    }

    /// The first slot to probe for `id`.
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "only the low bits index the table"
    )]
    fn slot(&self, id: &str) -> usize {
        let mut hash = 0xCBF2_9CE4_8422_2325_u64;
        for byte in id.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3);
        }
        (hash ^ hash >> 29) as usize & self.mask
    }
}
