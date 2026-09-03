//! Base-64 coding, as the representations that use it require.
//!
//! RFC 9804 §4.5 encodes one octet string in base 64, and §6.1 encodes a whole
//! canonical S-expression that way. Both cite [RFC 4648] for the coding
//! itself, so this is the standard alphabet, with `+` and `/` as the last two
//! characters and `=` as padding.
//!
//! [RFC 4648]: https://www.rfc-editor.org/rfc/rfc4648.html

use std::io;

use crate::syntax;

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

/// Decodes base 64, passing over the whitespace §4.5 and §6.1 allow inside it.
///
/// Returns the offset within `input` of the character that made decoding
/// impossible, or the length of `input` where the encoding ended in the middle
/// of a group.
///
/// The bits an encoder leaves zero in the last character of a group that is
/// not whole have to be zero here as well. Anything else would give one octet
/// string two encodings, which a representation that gets signed cannot
/// afford. Padding is required to fill the group it appears in, and, as RFC
/// 4648 §3.2 allows, an encoding that stops on a group boundary without
/// padding is accepted.
pub(crate) fn decode(input: &[u8]) -> Result<Vec<u8>, usize> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut group = [0u8; 4];
    let mut len = 0;
    let mut padding = 0;
    let mut last = 0;
    let mut ended = false;

    for (offset, &character) in input.iter().enumerate() {
        if syntax::is_whitespace(character) {
            continue;
        }

        if ended {
            return Err(offset);
        }

        if character == b'=' {
            // Padding stands in for the characters a whole group would have
            // had, and two characters are the fewest that stand for an octet.
            if len + padding < 2 {
                return Err(offset);
            }

            padding += 1;

            if len + padding == 4 {
                decode_group(&group[..len], &mut out, last)?;
                len = 0;
                padding = 0;
                ended = true;
            }

            continue;
        }

        if padding > 0 {
            return Err(offset);
        }

        group[len] = value_of(character).ok_or(offset)?;
        len += 1;
        last = offset;

        if len == 4 {
            decode_group(&group, &mut out, last)?;
            len = 0;
        }
    }

    if padding > 0 {
        return Err(input.len());
    }

    decode_group(&group[..len], &mut out, last)?;

    Ok(out)
}

/// Decodes one group of two to four characters into one to three octets.
///
/// A group of one character stands for no octet at all, and one of none is
/// what a whole encoding ends on, so the first is an error and the second is
/// nothing to do. The offset is where to report either failure.
fn decode_group(group: &[u8], out: &mut Vec<u8>, offset: usize) -> Result<(), usize> {
    match *group {
        [] => {}
        [first, second] => {
            if second & 0x0f != 0 {
                return Err(offset);
            }

            out.push((first << 2) | (second >> 4));
        }
        [first, second, third] => {
            if third & 0x03 != 0 {
                return Err(offset);
            }

            out.push((first << 2) | (second >> 4));
            out.push((second << 4) | (third >> 2));
        }
        [first, second, third, fourth] => {
            out.push((first << 2) | (second >> 4));
            out.push((second << 4) | (third >> 2));
            out.push((third << 6) | fourth);
        }
        _ => return Err(offset),
    }

    Ok(())
}

/// Returns what one base-64 character is worth.
fn value_of(character: u8) -> Option<u8> {
    let value = match character {
        b'A'..=b'Z' => character - b'A',
        b'a'..=b'z' => character - b'a' + 26,
        b'0'..=b'9' => character - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    };

    Some(value)
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
    fn writer_flushes_the_writer_beneath_it() {
        struct Counting {
            flushes: usize,
        }

        impl io::Write for Counting {
            fn write(&mut self, octets: &[u8]) -> io::Result<usize> {
                Ok(octets.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                self.flushes += 1;

                Ok(())
            }
        }

        let mut writer = Writer::new(Counting { flushes: 0 });

        writer.write_all(b"foo").unwrap();
        writer.flush().unwrap();

        assert_eq!(writer.inner.flushes, 1);
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

    // decode

    #[test]
    fn decode_reads_back_the_rfc_4648_test_vectors() {
        for (octets, encoded) in VECTORS {
            assert_eq!(decode(encoded.as_bytes()).unwrap(), octets.as_bytes());
        }
    }

    #[test]
    fn decode_reads_back_what_encode_into_wrote() {
        let octets: Vec<u8> = (0..=255).collect();

        for len in 0..octets.len() {
            let mut encoded = String::new();

            encode_into(&octets[..len], &mut encoded);

            assert_eq!(decode(encoded.as_bytes()).unwrap(), octets[..len]);
        }
    }

    #[test]
    fn decode_reads_the_whole_alphabet() {
        let mut encoded = String::new();

        for (value, &character) in ALPHABET.iter().enumerate() {
            assert_eq!(value_of(character), Some(value as u8));

            encoded.push(char::from(character));
        }

        assert_eq!(decode(encoded.as_bytes()).unwrap().len(), 48);
    }

    #[test]
    fn decode_passes_over_whitespace_wherever_it_stands() {
        for encoded in ["Zm9v", " Zm9v", "Zm 9v", "Zm9v ", "Z\r\nm\t9\x0bv\x0c"] {
            assert_eq!(decode(encoded.as_bytes()).unwrap(), b"foo");
        }

        assert_eq!(decode(b"Zm 8 =").unwrap(), b"fo");
    }

    #[test]
    fn decode_reads_nothing_as_nothing() {
        assert_eq!(decode(b"").unwrap(), b"");
        assert_eq!(decode(b"  ").unwrap(), b"");
    }

    #[test]
    fn decode_accepts_a_group_left_unpadded() {
        assert_eq!(decode(b"Zm8").unwrap(), b"fo");
        assert_eq!(decode(b"Zg").unwrap(), b"f");
    }

    #[test]
    fn decode_refuses_a_character_that_is_not_base_64() {
        assert_eq!(decode(b"Zm9*"), Err(3));
        assert_eq!(decode(b"-m9v"), Err(0));
        assert_eq!(decode(b"Zm9v!"), Err(4));
    }

    #[test]
    fn decode_refuses_a_group_of_one_character() {
        assert_eq!(decode(b"Z"), Err(0));
        assert_eq!(decode(b"Zm9vZ"), Err(4));
    }

    #[test]
    fn decode_refuses_padding_that_stands_for_too_much() {
        assert_eq!(decode(b"Z==="), Err(1));
        assert_eq!(decode(b"===="), Err(0));
        assert_eq!(decode(b"="), Err(0));
    }

    #[test]
    fn decode_refuses_padding_that_leaves_its_group_unfilled() {
        assert_eq!(decode(b"Zg="), Err(3));
        assert_eq!(decode(b"Zm9vZg="), Err(7));
    }

    #[test]
    fn decode_refuses_anything_after_a_padded_group() {
        assert_eq!(decode(b"Zg==Zg=="), Err(4));
        assert_eq!(decode(b"Zg==v"), Err(4));
    }

    #[test]
    fn decode_refuses_a_character_after_padding_within_a_group() {
        assert_eq!(decode(b"Zm=v"), Err(3));
    }

    #[test]
    fn decode_refuses_bits_an_encoder_would_have_left_zero() {
        // "Zg==" is "f", and only the four low bits of the second character
        // may be zero for the encoding to be the one an encoder writes.
        assert_eq!(decode(b"Zg=="), Ok(b"f".to_vec()));
        assert_eq!(decode(b"Zh=="), Err(1));
        assert_eq!(decode(b"Zm8="), Ok(b"fo".to_vec()));
        assert_eq!(decode(b"Zm9="), Err(2));
    }

    #[test]
    fn decode_refuses_the_same_bits_in_a_group_left_unpadded() {
        assert_eq!(decode(b"Zh"), Err(1));
        assert_eq!(decode(b"Zm9"), Err(2));
    }
}
