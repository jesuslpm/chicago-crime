//! Source fingerprint computed while the CSV is streamed (decision G4): no second pass.

use std::fmt::Write as _;
use std::io::{self, Read};

use sha2::{Digest, Sha256};

/// SHA-256 and size of everything read from a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// Lowercase hexadecimal SHA-256.
    pub sha256: String,
    pub size_bytes: u64,
}

/// Reader that hashes and counts every byte that passes through it.
pub struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
    size_bytes: u64,
}

impl<R: Read> HashingReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            size_bytes: 0,
        }
    }

    /// Reads whatever is left up to the end of the source, so the fingerprint always covers the
    /// whole source, and returns it.
    pub fn finish(mut self) -> io::Result<Fingerprint> {
        io::copy(&mut self, &mut io::sink())?;
        let digest = self.hasher.finalize();
        let mut sha256 = String::with_capacity(digest.len() * 2);
        for byte in digest.iter() {
            write!(sha256, "{byte:02x}").expect("writing to a String cannot fail");
        }
        Ok(Fingerprint {
            sha256,
            size_bytes: self.size_bytes,
        })
    }
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.hasher.update(&buf[..read]);
        self.size_bytes += read as u64;
        Ok(read)
    }
}
