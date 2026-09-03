//! Base-64 coding, as the representations that use it require.
//!
//! RFC 9804 §4.5 encodes one octet string in base 64, and §6.1 encodes a whole
//! canonical S-expression that way. Both cite [RFC 4648] for the coding
//! itself, so this is the standard alphabet, with `+` and `/` as the last two
//! characters and `=` as padding.
//!
//! [RFC 4648]: https://www.rfc-editor.org/rfc/rfc4648.html

use std::io;

/// The standard alphabet of RFC 4648 §4, in value order.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Returns how many characters `octets` octets encode to, padding included.
pub(crate) fn encoded_len(octets: usize) -> usize {
    // Every group of three octets, whole or partial, becomes four characters.
    octets.div_ceil(3).saturating_mul(4)
}

/// Appends the base-64 encoding of `octets` to `out`.
pub(crate) fn encode_into(octets: &[u8], out: &mut String) {
    out.reserve(encoded_len(octets.len()));

    for group in octets.chunks(3) {
        for character in encode_group(group) {
            out.push(char::from(character));
        }
    }
}

/// Encodes one group of one to three octets into four characters, padding a
/// partial group with `=` as RFC 4648 §4 requires.
fn encode_group(group: &[u8]) -> [u8; 4] {
    let mut bits = 0u32;

    for (index, &octet) in group.iter().enumerate() {
        bits |= u32::from(octet) << (16 - 8 * index);
    }

    let mut characters = [b'='; 4];

    for (index, character) in characters.iter_mut().take(group.len() + 1).enumerate() {
        *character = ALPHABET[((bits >> (18 - 6 * index)) & 0x3f) as usize];
    }

    characters
}

/// A writer that encodes in base 64 whatever is written to it.
///
/// Octets arrive in whatever sizes the caller writes them, and base 64 codes
/// three at a time, so up to two octets wait here between writes. [`finish`]
/// codes those last octets and pads them, and must be called for the encoding
/// to be complete.
///
/// [`finish`]: Self::finish
pub(crate) struct Writer<W> {
    inner: W,
    pending: [u8; 3],
    pending_len: usize,
}

impl<W: io::Write> Writer<W> {
    /// Creates a writer that encodes into `inner`.
    pub(crate) fn new(inner: W) -> Self {
        Self {
            inner,
            pending: [0; 3],
            pending_len: 0,
        }
    }

    /// Codes the octets still waiting, pads them, and gives back the writer
    /// they were coded into.
    ///
    /// # Errors
    ///
    /// Returns whatever error the underlying writer returns.
    pub(crate) fn finish(mut self) -> io::Result<W> {
        if self.pending_len > 0 {
            let group = encode_group(&self.pending[..self.pending_len]);
            self.inner.write_all(&group)?;
            self.pending_len = 0;
        }

        Ok(self.inner)
    }
}

impl<W: io::Write> io::Write for Writer<W> {
    fn write(&mut self, octets: &[u8]) -> io::Result<usize> {
        if octets.is_empty() {
            return Ok(0);
        }

        // Fill the waiting group first, and code it once it is whole.
        if self.pending_len > 0 {
            let taken = octets.len().min(3 - self.pending_len);
            self.pending[self.pending_len..self.pending_len + taken]
                .copy_from_slice(&octets[..taken]);
            self.pending_len += taken;

            if self.pending_len < 3 {
                return Ok(taken);
            }

            let group = encode_group(&self.pending);
            self.inner.write_all(&group)?;
            self.pending_len = 0;

            return Ok(taken);
        }

        let whole = octets.len() - octets.len() % 3;

        if whole == 0 {
            self.pending[..octets.len()].copy_from_slice(octets);
            self.pending_len = octets.len();

            return Ok(octets.len());
        }

        for group in octets[..whole].chunks(3) {
            self.inner.write_all(&encode_group(group))?;
        }

        Ok(whole)
    }

    /// Flushes the underlying writer, leaving any waiting octets waiting.
    ///
    /// Fewer than three octets cannot be coded without padding, and padding
    /// ends the encoding, so they stay here until [`Writer::finish`].
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// The test vectors of RFC 4648 §10.
    const VECTORS: &[(&str, &str)] = &[
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ];

    fn streamed(octets: &[u8], chunk: usize) -> String {
        let mut writer = Writer::new(Vec::new());

        for part in octets.chunks(chunk.max(1)) {
            writer.write_all(part).unwrap();
        }

        String::from_utf8(writer.finish().unwrap()).unwrap()
    }

    // encoded_len

    #[test]
    fn encoded_len_counts_four_characters_per_group() {
        assert_eq!(encoded_len(0), 0);
        assert_eq!(encoded_len(1), 4);
        assert_eq!(encoded_len(3), 4);
        assert_eq!(encoded_len(4), 8);
        assert_eq!(encoded_len(6), 8);
    }

    #[test]
    fn encoded_len_matches_what_is_encoded() {
        for len in 0..64 {
            let mut out = String::new();

            encode_into(&vec![0xa5; len], &mut out);

            assert_eq!(out.len(), encoded_len(len));
        }
    }

    // encode_into

    #[test]
    fn encode_into_matches_the_rfc_4648_test_vectors() {
        for (octets, expected) in VECTORS {
            let mut out = String::new();

            encode_into(octets.as_bytes(), &mut out);

            assert_eq!(out, *expected);
        }
    }

    #[test]
    fn encode_into_uses_the_standard_alphabet() {
        let mut out = String::new();

        encode_into(&[0xff, 0xef, 0xbe], &mut out);

        assert_eq!(out, "/+++");
    }

    #[test]
    fn encode_into_appends_to_what_is_there() {
        let mut out = String::from("{");

        encode_into(b"foo", &mut out);

        assert_eq!(out, "{Zm9v");
    }

    #[test]
    fn encode_into_pads_a_partial_group() {
        for (octets, expected) in VECTORS {
            let mut out = String::new();

            encode_into(octets.as_bytes(), &mut out);

            let padding = 3 - (octets.len() + 2) % 3 - 1;

            assert_eq!(out.bytes().filter(|&c| c == b'=').count(), padding);
            assert_eq!(out, *expected);
        }
    }

    // Writer

    #[test]
    fn writer_matches_the_rfc_4648_test_vectors() {
        for (octets, expected) in VECTORS {
            assert_eq!(streamed(octets.as_bytes(), 64), *expected);
        }
    }

    #[test]
    fn writer_encodes_the_same_whatever_the_write_sizes() {
        let octets: Vec<u8> = (0..=255).collect();
        let mut expected = String::new();

        encode_into(&octets, &mut expected);

        for chunk in [1, 2, 3, 4, 5, 7, 64, 256] {
            assert_eq!(streamed(&octets, chunk), expected);
        }
    }

    #[test]
    fn writer_holds_a_partial_group_until_it_finishes() {
        let mut writer = Writer::new(Vec::new());

        writer.write_all(b"fo").unwrap();

        assert!(writer.inner.is_empty());
        assert_eq!(String::from_utf8(writer.finish().unwrap()).unwrap(), "Zm8=");
    }

    #[test]
    fn writer_finishes_an_empty_encoding_as_nothing() {
        assert_eq!(streamed(b"", 1), "");
    }

    #[test]
    fn writer_reports_a_failure_of_the_writer() {
        struct Full;

        impl io::Write for Full {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::StorageFull))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut writer = Writer::new(Full);

        assert!(writer.write_all(b"foo").is_err());
        assert!(Writer::new(Full).write_all(b"f").is_ok());
        assert!(Writer::new(Full).finish().is_ok());
    }
}
