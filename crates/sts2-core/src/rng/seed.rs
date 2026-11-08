//! Seed strings and stream names: `StringHelper.GetDeterministicHashCode` (XXH64),
//! `StringHelper.SnakeCase` and `SeedHelper.CanonicalizeSeed`.

const P1: u64 = 0x9E37_79B1_85EB_CA87;
const P2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const P3: u64 = 0x1656_67B1_9E37_79F9;
const P4: u64 = 0x85EB_CA77_C2B2_AE63;
const P5: u64 = 0x27D4_EB2F_1656_67C5;

const fn round(acc: u64, lane: u64) -> u64 {
    acc.wrapping_add(lane.wrapping_mul(P2))
        .rotate_left(31)
        .wrapping_mul(P1)
}

const fn merge_round(acc: u64, value: u64) -> u64 {
    (acc ^ round(0, value)).wrapping_mul(P1).wrapping_add(P4)
}

/// Standard XXH64 with seed 0 over raw bytes.
fn xxh64_bytes(input: &[u8]) -> u64 {
    let (stripes, tail) = input.as_chunks::<32>();
    let mut hash = if stripes.is_empty() {
        P5
    } else {
        let mut acc = [P1.wrapping_add(P2), P2, 0, P1.wrapping_neg()];
        for stripe in stripes {
            for (lane, acc) in stripe.as_chunks::<8>().0.iter().zip(&mut acc) {
                *acc = round(*acc, u64::from_le_bytes(*lane));
            }
        }
        let [a, b, c, d] = acc;
        let hash = a
            .rotate_left(1)
            .wrapping_add(b.rotate_left(7))
            .wrapping_add(c.rotate_left(12))
            .wrapping_add(d.rotate_left(18));
        acc.into_iter().fold(hash, merge_round)
    };
    hash = hash.wrapping_add(input.len() as u64);

    let (words, tail) = tail.as_chunks::<8>();
    for word in words {
        hash = (hash ^ round(0, u64::from_le_bytes(*word)))
            .rotate_left(27)
            .wrapping_mul(P1)
            .wrapping_add(P4);
    }
    let (half_words, tail) = tail.as_chunks::<4>();
    // At most one 4-byte word remains.
    for half in half_words {
        hash = (hash ^ u64::from(u32::from_le_bytes(*half)).wrapping_mul(P1))
            .rotate_left(23)
            .wrapping_mul(P2)
            .wrapping_add(P3);
    }
    for &byte in tail {
        hash = (hash ^ u64::from(byte).wrapping_mul(P5))
            .rotate_left(11)
            .wrapping_mul(P1);
    }

    hash ^= hash >> 33;
    hash = hash.wrapping_mul(P2);
    hash ^= hash >> 29;
    hash = hash.wrapping_mul(P3);
    hash ^ (hash >> 32)
}

/// `StringHelper.GetDeterministicHashCode`: XXH64 (seed 0) of the UTF-8 bytes of `s`.
#[must_use]
pub fn xxh64(s: &str) -> u64 {
    xxh64_bytes(s.as_bytes())
}

/// `StringHelper.SnakeCase`: trims, inserts `_` before every ASCII capital that follows an
/// ASCII letter or digit, then lower-cases.
///
/// This is the regex `([A-Za-z0-9]|\G(?!^))([A-Z])` -> `$1_$2`: the `\G` branch only fires
/// right after a previous match, whose last character is a capital, so it is covered by the
/// same rule. Lower-casing is ASCII-only, which is exact for the identifiers the game passes.
#[must_use]
pub fn snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_alnum = false;
    for c in s.trim().chars() {
        if c.is_ascii_uppercase() && prev_alnum {
            out.push('_');
        }
        prev_alnum = c.is_ascii_alphanumeric();
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// `SeedHelper.CanonicalizeSeed`: upper-case, `O` -> `0`, `I` -> `1`, then trim.
///
/// `ToUpperInvariant` maps one character to one character (`ß` stays `ß`), so characters
/// whose full upper-case form is longer are kept as they are, and like all .NET invariant
/// casing it leaves the dotless `ı` alone (Rust would upper-case it to `I`, then `1`).
#[must_use]
pub fn canonicalize_seed(seed: &str) -> String {
    let upper: String = seed
        .chars()
        .map(|c| {
            if c == 'ı' {
                return c;
            }
            let mut mapped = c.to_uppercase();
            match (mapped.next(), mapped.next()) {
                (Some(single), None) => single,
                _ => c,
            }
        })
        .map(|c| match c {
            'O' => '0',
            'I' => '1',
            other => other,
        })
        .collect();
    upper.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xxh64_known_answer() {
        assert_eq!(xxh64(""), 0xEF46_DB37_51D8_E999);
    }
}
