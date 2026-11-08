//! Deterministic Slay the Spire 2 engine core.
#![warn(clippy::allow_attributes)]
#![warn(missing_docs)]
#![warn(clippy::as_conversions)]

/// `impl Clone` field by field, with a `clone_from` that calls each field's own `clone_from`: a
/// `Vec` keeps its buffer, so copying a state into one of the same shape (a search's scratch
/// copy, [`Combat::clone_from`]) allocates nothing. `clone` is what `derive(Clone)` would write.
/// A field marked `: copy` is a `Vec` of `Copy` items, refilled by [`copy_vec_from`]. With
/// `checked` after the braces, debug and `regress` builds assert that a `clone_from` copy equals
/// its source (a field left out of the copy, or a derived `clone_from`, would show here). The
/// patterns name every field (no `..`), so a field added to the struct fails to compile here
/// until it is listed.
macro_rules! impl_clone_by_fields {
    ($ty:ident { $($field:ident $(: $mode:ident)?),+ $(,)? }) => {
        impl Clone for $ty {
            #[inline]
            fn clone(&self) -> Self {
                let Self { $($field),+ } = self;
                Self { $($field: $field.clone()),+ }
            }

            #[inline]
            fn clone_from(&mut self, source: &Self) {
                let Self { $($field),+ } = source;
                $(clone_field_from!(self.$field, $field $(, $mode)?);)+
            }
        }
    };
    // `checked`: debug and `regress` builds also assert that the copy equals its source.
    ($ty:ident { $($field:ident $(: $mode:ident)?),+ $(,)? } checked) => {
        impl Clone for $ty {
            #[inline]
            fn clone(&self) -> Self {
                let Self { $($field),+ } = self;
                Self { $($field: $field.clone()),+ }
            }

            #[inline]
            fn clone_from(&mut self, source: &Self) {
                {
                    let Self { $($field),+ } = source;
                    $(clone_field_from!(self.$field, $field $(, $mode)?);)+
                }
                debug_assert!(
                    self == source,
                    concat!("`", stringify!($ty), "::clone_from` made an unequal copy")
                );
            }
        }
    };
}

/// `const fn index(self) -> usize` on fieldless enums (the variant's discriminant) and on the
/// `u32` id newtypes (the id), for the tables and bit sets they index: the one place those
/// conversions are written as casts.
macro_rules! impl_index {
    ($($ty:ty),+ $(,)?) => {
        $(impl $ty {
            /// The discriminant, as a table index.
            #[must_use]
            #[inline]
            #[expect(clippy::as_conversions, reason = "a fieldless enum's discriminant")]
            pub const fn index(self) -> usize {
                self as usize
            }
        })+
    };
    ($($ty:ty),+ $(,)?; id) => {
        $(impl $ty {
            /// The id, as an index.
            #[must_use]
            #[inline]
            #[expect(
                clippy::as_conversions,
                reason = "a `u32` id; `usize` is at least 32 bits on every supported target"
            )]
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        })+
    };
}

/// One field of [`impl_clone_by_fields`].
macro_rules! clone_field_from {
    ($dst:expr, $src:expr) => {
        $dst.clone_from($src)
    };
    ($dst:expr, $src:expr, copy) => {
        $crate::copy_vec_from(&mut $dst, $src)
    };
}

/// `dst.clone_from(src)` for a list of `Copy` items, without the `memcpy` call when `src` is
/// empty (most of a fight's lists are, and a search copies every list of the state).
#[inline]
pub(crate) fn copy_vec_from<T: Copy>(dst: &mut Vec<T>, src: &[T]) {
    dst.clear();
    if !src.is_empty() {
        dst.extend_from_slice(src);
    }
}

pub mod capacity;
pub mod combat;
pub mod content;
pub mod decimal;
pub mod digest;
pub mod rng;
pub mod run;
pub mod shuffle;
pub mod text;

pub use combat::{Action, Combat, CombatConfig, Status};
pub use content::characters::Character;
pub use decimal::Decimal;
pub use rng::Rng;
