//! Project-wide hash collections.
//!
//! `hashbrown`'s SwissTable with the crate-default foldhash. Preferred over
//! `std::collections` because the application's keys are never
//! attacker-controlled (entry names, indices, pixels), and foldhash drops
//! SipHash-1-3's cost — measured ~1.8–2.5× on the app's key shapes in
//! `examples/benchmark_hashmaps.rs` (local-only harness). `hashbrown` is
//! already in the dependency tree through `quick_cache`/`indexmap`, so this
//! adds no new compile units. Every user-visible ordering is produced by
//! explicit `Vec` sorts or `BTreeMap` output, so iteration order never leaks.
//! Decision record: `docs/hashmap-crate.md`.

pub use hashbrown::Equivalent;
pub use hashbrown::hash_map::{Entry, EntryRef};
pub use hashbrown::{HashMap, HashSet};

use compact_str::CompactString;
use std::hash::{Hash, Hasher};

/// Lowercases `s` into a [`CompactString`] using a fast in-place ASCII path
/// before falling back to `std::str::to_lowercase`.
///
/// Checking `is_ascii()` first copies once into the 24-byte inline buffer and
/// lowercases in place with zero heap allocation. The non-ASCII fallback uses
/// `std::str::to_lowercase()` (never `CompactString::from_str_to_lowercase`,
/// whose 16-byte ASCII chunker can misclassify a trailing Greek `'Σ'` at a
/// 16-byte boundary) so stored keys always match [`LowerStr`]'s `Hash` and
/// `Equivalent` contract across all Unicode inputs.
#[inline]
pub fn compact_to_lowercase(s: &str) -> CompactString {
    if s.is_ascii() {
        let mut out = CompactString::new(s);
        out.make_ascii_lowercase();
        out
    } else {
        CompactString::from(s.to_lowercase())
    }
}

/// Zero-allocation lowercase lookup key for hash collections whose stored
/// keys are already lowercased (`str::to_lowercase`).
///
/// Uses `hashbrown`'s [`Equivalent`] trait so `get`, `contains`, `take`,
/// and `entry_ref` can probe `HashMap<String, _>`, `HashMap<CompactString, _>`,
/// and `HashSet<_>` without allocating a temporary lowercase `String` on the
/// common ASCII path.
#[derive(Debug, Clone, Copy)]
pub struct LowerStr<'a>(pub &'a str);

impl Hash for LowerStr<'_> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let s = self.0;
        if !s
            .as_bytes()
            .iter()
            .any(|&b| b >= 0x80 || b.is_ascii_uppercase())
        {
            s.hash(state);
            return;
        }
        hash_lower_str_slow(s, state);
    }
}

fn hash_lower_str_slow<H: Hasher>(s: &str, state: &mut H) {
    let bytes = s.as_bytes();
    if bytes.len() <= 64 && s.is_ascii() {
        let mut buf = [0u8; 64];
        for (dst, &b) in buf.iter_mut().zip(bytes) {
            *dst = b.to_ascii_lowercase();
        }
        if let Ok(lower) = std::str::from_utf8(&buf[..bytes.len()]) {
            lower.hash(state);
            return;
        }
    } else if s.is_ascii() {
        s.to_ascii_lowercase().hash(state);
        return;
    }
    s.to_lowercase().hash(state);
}

#[inline]
fn eq_lowercased(input: &str, stored: &str) -> bool {
    if input.is_ascii() {
        input.len() == stored.len()
            && input
                .bytes()
                .zip(stored.bytes())
                .all(|(a, b)| a.to_ascii_lowercase() == b)
    } else {
        input.to_lowercase() == stored
    }
}

impl Equivalent<str> for LowerStr<'_> {
    #[inline]
    fn equivalent(&self, key: &str) -> bool {
        eq_lowercased(self.0, key)
    }
}

impl Equivalent<&str> for LowerStr<'_> {
    #[inline]
    fn equivalent(&self, key: &&str) -> bool {
        eq_lowercased(self.0, key)
    }
}

impl Equivalent<String> for LowerStr<'_> {
    #[inline]
    fn equivalent(&self, key: &String) -> bool {
        eq_lowercased(self.0, key.as_str())
    }
}

impl Equivalent<CompactString> for LowerStr<'_> {
    #[inline]
    fn equivalent(&self, key: &CompactString) -> bool {
        eq_lowercased(self.0, key.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::BuildHasher;

    #[test]
    fn lower_str_matches_to_lowercase_hash_and_lookup_across_ascii_and_unicode() {
        let mut map: HashMap<String, usize> = HashMap::new();
        let cases = [
            ("", ""),
            ("player.dff", "PLAYER.DFF"),
            ("Mixed_Case_Texture.TXD", "mixed_case_texture.txd"),
            // >64-byte lowercase and mixed-case ASCII strings (beyond stack buffer).
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.dff",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.DFF",
            ),
            (
                "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789.txd",
                "abcdefghijklmnopqrstuvwxyz0123456789_abcdefghijklmnopqrstuvwxyz0123456789.txd",
            ),
            // Non-ASCII Unicode case folding, including Kelvin sign U+212A -> "k"
            // and dotted capital I U+0130 -> "i\u{307}".
            ("ÜBER_Model.dff", "über_model.dff"),
            ("\u{212A}.txd", "K.txd"),
            ("\u{0130}stanbul.col", "i\u{0307}stanbul.col"),
        ];

        for (index, (input, alias)) in cases.iter().enumerate() {
            let lowered = input.to_lowercase();
            assert_eq!(
                map.hasher().hash_one(LowerStr(input)),
                map.hasher().hash_one(&lowered),
                "hash mismatch for {input:?}"
            );
            assert_eq!(
                map.hasher().hash_one(LowerStr(alias)),
                map.hasher().hash_one(&lowered),
                "hash mismatch for alias {alias:?}"
            );
            map.insert(lowered, index);
        }

        for (index, (input, alias)) in cases.iter().enumerate() {
            assert_eq!(map.get(&LowerStr(input)), Some(&index), "lookup {input:?}");
            assert_eq!(map.get(&LowerStr(alias)), Some(&index), "lookup {alias:?}");
        }
        assert_eq!(map.get(&LowerStr("k.txd")), Some(&6));
    }

    #[test]
    fn lower_str_does_not_false_match_uppercase_stored_keys() {
        let mut set: HashSet<String> = HashSet::new();
        set.insert(String::from("UPPER.DFF"));
        // Contract: LowerStr("upper.dff") hashes "upper.dff", never "UPPER.DFF",
        // so Equivalent must return false against an un-lowercased stored key.
        assert!(!LowerStr("upper.dff").equivalent(&String::from("UPPER.DFF")));
        assert!(!set.contains(&LowerStr("upper.dff")));
    }

    #[test]
    fn lower_str_works_with_entry_ref_compact_string_and_set_take() {
        let mut map: HashMap<CompactString, u32> = HashMap::new();
        for name in ["Banshee.txd", "BANSHEE.TXD", "banshee.txd"] {
            match map.entry_ref(&LowerStr(name)) {
                EntryRef::Occupied(mut slot) => *slot.get_mut() += 1,
                EntryRef::Vacant(slot) => {
                    slot.insert_with_key(compact_to_lowercase(name), 1);
                }
            }
        }
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&LowerStr("BaNsHeE.TxD")), Some(&3));

        let mut wanted: HashSet<String> = HashSet::new();
        wanted.insert(String::from("road_01.txd"));
        assert_eq!(
            wanted.take(&LowerStr("ROAD_01.TXD")),
            Some(String::from("road_01.txd"))
        );
        assert!(wanted.is_empty());
    }

    #[test]
    fn compact_to_lowercase_matches_std_and_inlines_short_strings() {
        let cases = [
            "",
            "cj",
            "Police_Car_01.DFF",
            "24_Byte_Exact_Name_12345",
            "ÜBER_Model.dff",
            "ὈΔΥΣΣΕΎΣ",
            "\u{212A}.txd",
            "\u{0130}stanbul.col",
            // 16-byte ASCII prefix + word-final Greek capital sigma: catches
            // compact_str 0.10.0's 16-byte chunk boundary bug against std.
            "aaaaaaaaaaaaaaaaΣ",
            "0123456789abcdefΣ_1.txd",
        ];
        let mut map: HashMap<CompactString, usize> = HashMap::new();
        for (idx, input) in cases.iter().enumerate() {
            let lowered = compact_to_lowercase(input);
            assert_eq!(lowered.as_str(), input.to_lowercase());
            if lowered.len() <= 24 {
                assert!(!lowered.is_heap_allocated(), "{input:?} spilled to heap");
            }
            if let EntryRef::Vacant(slot) = map.entry_ref(&LowerStr(input)) {
                slot.insert_with_key(lowered, idx);
            }
            assert_eq!(map.get(&LowerStr(input)), Some(&idx));
        }
    }
}
