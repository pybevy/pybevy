//! Versioned, framed SHA-256 support for stable asset payload digests.

use sha2::{Digest, Sha256};

/// Incremental canonical content hasher.
///
/// Every value is framed as `(tag length, tag, payload length, payload)`, so
/// concatenated values and absent/empty variants cannot collide structurally.
pub struct CanonicalContentHasher {
    hasher: Sha256,
}

impl CanonicalContentHasher {
    pub fn new(domain: &str, version: u32) -> Self {
        let mut this = Self {
            hasher: Sha256::new(),
        };
        this.write("domain", domain.as_bytes());
        this.write("version", &version.to_le_bytes());
        this
    }

    pub fn write(&mut self, tag: &str, payload: &[u8]) {
        self.hasher.update((tag.len() as u64).to_le_bytes());
        self.hasher.update(tag.as_bytes());
        self.hasher.update((payload.len() as u64).to_le_bytes());
        self.hasher.update(payload);
    }

    pub fn finish(self) -> String {
        let digest = self.hasher.finalize();
        let mut output = String::with_capacity(digest.len() * 2);
        for byte in digest {
            use std::fmt::Write;
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::CanonicalContentHasher;

    #[test]
    fn framing_distinguishes_concatenation_and_empty_values() {
        let mut left = CanonicalContentHasher::new("test", 1);
        left.write("a", b"bc");

        let mut right = CanonicalContentHasher::new("test", 1);
        right.write("ab", b"c");
        assert_ne!(left.finish(), right.finish());

        let mut absent = CanonicalContentHasher::new("test", 1);
        absent.write("none", &[]);
        let mut empty = CanonicalContentHasher::new("test", 1);
        empty.write("some", &[]);
        assert_ne!(absent.finish(), empty.finish());
    }
}
