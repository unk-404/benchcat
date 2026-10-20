//! Hashers for maps whose keys are already dense integers.
//!
//! Hashing a glyph id or a codepoint before using it as a key buys nothing: the
//! value is already unique and small. [`BuildIdentity`] forwards such keys
//! unchanged, so the map only pays for the table probe, not for a hash.

use std::hash::{BuildHasher, Hasher};

#[derive(Default)]
pub struct IdentityHasher {
    state: u64,
}

impl Hasher for IdentityHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    /// Fallback for byte-slice keys; integer keys use the `write_uNN` methods.
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.state = hash;
    }

    fn write_u8(&mut self, value: u8) {
        self.state = u64::from(value);
    }

    fn write_u16(&mut self, value: u16) {
        self.state = u64::from(value);
    }

    fn write_u32(&mut self, value: u32) {
        self.state = u64::from(value);
    }

    fn write_u64(&mut self, value: u64) {
        self.state = value;
    }

    fn write_usize(&mut self, value: usize) {
        self.state = value as u64;
    }
}

#[derive(Default, Clone, Copy)]
pub struct BuildIdentity;

impl BuildHasher for BuildIdentity {
    type Hasher = IdentityHasher;

    fn build_hasher(&self) -> IdentityHasher {
        IdentityHasher::default()
    }
}

#[cfg(test)]
mod tests {
    use super::BuildIdentity;

    #[test]
    fn identity_hasher_maps_integer_keys() {
        let mut map: hashbrown::HashMap<u32, &str, BuildIdentity> = hashbrown::HashMap::default();
        map.insert(7, "seven");
        map.insert(65, "A");
        assert_eq!(map.get(&65), Some(&"A"));
        assert_eq!(map.len(), 2);
    }
}
