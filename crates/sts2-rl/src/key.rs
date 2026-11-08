//! Exact cache keys: a value's derived `Hash` written out as words instead of mixed into a
//! hash, so two keys are equal only when the values hashed the same field by field (no
//! collisions to argue about).

use std::hash::{Hash, Hasher};

/// A [`Hasher`] that appends every write to a word list; [`Hasher::finish`] is unused.
#[derive(Debug)]
pub struct Words<'a>(pub &'a mut Vec<u64>);

impl Words<'_> {
    /// Appends `value`'s words.
    pub fn add(&mut self, value: &impl Hash) {
        value.hash(self);
    }
}

impl Hasher for Words<'_> {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, bytes: &[u8]) {
        // The length first, so "ab" and "ab\0" differ although both pad to one word.
        self.0.push(bytes.len() as u64);
        for chunk in bytes.chunks(8) {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.0.push(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, i: u8) {
        self.0.push(u64::from(i));
    }

    fn write_u16(&mut self, i: u16) {
        self.0.push(u64::from(i));
    }

    fn write_u32(&mut self, i: u32) {
        self.0.push(u64::from(i));
    }

    fn write_u64(&mut self, i: u64) {
        self.0.push(i);
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the low word, then the high word"
    )]
    fn write_u128(&mut self, i: u128) {
        self.0.push(i as u64);
        self.0.push((i >> 64) as u64);
    }

    fn write_usize(&mut self, i: usize) {
        self.0.push(i as u64);
    }

    fn write_i8(&mut self, i: i8) {
        self.write_u8(i.cast_unsigned());
    }

    fn write_i16(&mut self, i: i16) {
        self.write_u16(i.cast_unsigned());
    }

    fn write_i32(&mut self, i: i32) {
        self.write_u32(i.cast_unsigned());
    }

    fn write_i64(&mut self, i: i64) {
        self.write_u64(i.cast_unsigned());
    }

    fn write_i128(&mut self, i: i128) {
        self.write_u128(i.cast_unsigned());
    }

    fn write_isize(&mut self, i: isize) {
        self.write_usize(i.cast_unsigned());
    }
}
