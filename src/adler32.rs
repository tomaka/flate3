//! Implementation of the Adler32 hashing algorithm.

/// An Implementation of the Adler-32 checksum
#[derive(Clone, Copy)]
pub struct Adler32 {
    s1: u32,
    s2: u32,
}

impl Adler32 {
    /// Create a new hasher.
    pub fn new() -> Adler32 {
        Adler32 { s1: 1, s2: 0 }
    }

    /// Update the internal hasher with the bytes from `buf`.
    pub fn feed(&mut self, buf: &[u8]) {
        for &byte in buf {
            self.s1 = self.s1 + byte as u32;
            self.s2 = self.s1 + self.s2;

            self.s1 %= 65521;
            self.s2 %= 65521;
        }
    }

    /// Return the computed hash.
    pub fn checksum(self) -> u32 {
        (self.s2 << 16) | self.s1
    }
}
