//! The basic transport representation ([RFC 9804], §6.3).
//!
//! §6.3 admits two forms for moving an S-expression from one machine to
//! another. The first is the canonical representation itself, which
//! [`crate::encode::canonical`] writes. The second is that same canonical
//! representation encoded in base 64 and surrounded by braces, as §6.1 gives
//! it, which is what this module writes.
//!
//! The braced form spends four characters for every three octets, and buys
//! with them an encoding that survives a channel that would disturb raw
//! octets, since every character it emits is a base-64 character or a brace.
//! What is inside the braces is the canonical representation and nothing else,
//! so an S-expression has exactly one braced form, and a signature computed
//! over the canonical octets is unaffected by wrapping them this way.
//!
//! §6.1 allows whitespace inside the braces, which a reader ignores. Nothing
//! here emits any, so the output of a whole S-expression is one unbroken run
//! of characters. An application that wants it in lines of bounded length
//! breaks it up itself, and [`crate::decode`] accepts the result.
//!
//! [RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html

use std::io;

use crate::base64;
use crate::encode::canonical;
use crate::types::Sexp;

/// Returns the basic transport representation of the S-expression, as text.
///
/// Every character is a base-64 character or a brace, so the result is ASCII.
///
/// # Examples
///
/// ```
/// use sexp::{encode::transport, sexp};
///
/// let cert = sexp!["issuer", "bob"];
///
/// assert_eq!(transport::to_string(&cert), "{KDY6aXNzdWVyMzpib2Ip}");
/// ```
pub fn to_string(sexp: &Sexp) -> String {
    let canonical = canonical::to_vec(sexp);
    let mut out = String::with_capacity(encoded_len_of(canonical.len()));

    out.push('{');
    base64::encode_into(&canonical, &mut out);
    out.push('}');

    out
}

/// Returns the basic transport representation of the S-expression.
///
/// This is [`to_string`] as octets, for a caller that works in octets.
pub fn to_vec(sexp: &Sexp) -> Vec<u8> {
    to_string(sexp).into_bytes()
}

/// Writes the basic transport representation of the S-expression to `out`.
///
/// The canonical representation is coded as it is produced, so writing this
/// way never holds the whole of it in memory, as [`to_string`] does.
///
/// # Errors
///
/// Returns whatever error `out` returns, at the point it returns it, leaving
/// however much was written before that already written.
pub fn write(sexp: &Sexp, out: &mut impl io::Write) -> io::Result<()> {
    out.write_all(b"{")?;

    let mut encoder = base64::Writer::new(out);
    canonical::write(sexp, &mut encoder)?;

    encoder.finish()?.write_all(b"}")
}

/// Returns how many characters the basic transport representation occupies.
///
/// This is exact rather than an estimate, so an application that caps the size
/// of what it emits can measure before writing.
pub fn encoded_len(sexp: &Sexp) -> usize {
    encoded_len_of(canonical::encoded_len(sexp))
}

/// Returns how many characters wrap a canonical representation of `octets`
/// octets.
fn encoded_len_of(octets: usize) -> usize {
    // One character for each brace.
    base64::encoded_len(octets).saturating_add(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexp;
    use crate::types::Atom;

    /// A nesting deeper than any stack the tests may assume.
    const DEEP: usize = 100_000;

    /// Canonical representations and the braced form each one takes.
    const VECTORS: &[(&str, &str)] = &[
        ("()", "{KCk=}"),
        ("4:data", "{NDpkYXRh}"),
        ("(1:a)", "{KDE6YSk=}"),
        ("[4:hint]4:data", "{WzQ6aGludF00OmRhdGE=}"),
        ("(6:issuer3:bob)", "{KDY6aXNzdWVyMzpib2Ip}"),
    ];

    /// The S-expression each vector encodes, in the order the vectors give.
    fn subjects() -> Vec<Sexp> {
        vec![
            Sexp::list([]),
            Sexp::atom("data"),
            sexp!["a"],
            Sexp::from(Atom::new("data").with_hint("hint")),
            sexp!["issuer", "bob"],
        ]
    }

    /// Builds `depth` nested lists around one atom, without recursing.
    fn nested(depth: usize) -> Sexp {
        let mut sexp = Sexp::atom("data");

        for _ in 0..depth {
            sexp = Sexp::list([sexp]);
        }

        sexp
    }

    /// Drops an S-expression without recursing, by lifting every list's
    /// elements out before the list that held them is dropped.
    fn dismantle(sexp: Sexp) {
        let mut stack = vec![sexp];

        while let Some(node) = stack.pop() {
            if let Some(items) = node.into_list() {
                stack.extend(items);
            }
        }
    }

    // to_string

    #[test]
    fn to_string_wraps_the_canonical_representation() {
        for (vector, subject) in VECTORS.iter().zip(subjects()) {
            let (expected_canonical, expected) = vector;

            assert_eq!(canonical::to_vec(&subject), expected_canonical.as_bytes());
            assert_eq!(to_string(&subject), *expected);
        }
    }

    #[test]
    fn to_string_surrounds_the_encoding_with_braces() {
        for subject in subjects() {
            let encoded = to_string(&subject);

            assert!(encoded.starts_with('{'));
            assert!(encoded.ends_with('}'));
        }
    }

    #[test]
    fn to_string_emits_only_base_64_characters_and_braces() {
        let subject = Sexp::from(Atom::new([0x00, 0xff, 0x1b]).with_hint([0x7f]));
        let encoded = to_string(&subject);

        assert!(encoded.is_ascii());
        assert!(
            encoded[1..encoded.len() - 1]
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'='))
        );
    }

    #[test]
    fn to_string_emits_no_whitespace() {
        let subject = Sexp::atom(vec![b'x'; 300]);

        assert!(!to_string(&subject).bytes().any(|c| c.is_ascii_whitespace()));
    }

    #[test]
    fn to_string_agrees_with_equality() {
        for left in subjects() {
            for right in subjects() {
                assert_eq!(left == right, to_string(&left) == to_string(&right));
            }
        }
    }

    #[test]
    fn to_string_does_not_recurse() {
        let deep = nested(DEEP);

        assert_eq!(to_string(&deep).len(), encoded_len(&deep));

        dismantle(deep);
    }

    // to_vec

    #[test]
    fn to_vec_is_to_string_as_octets() {
        for subject in subjects() {
            assert_eq!(to_vec(&subject), to_string(&subject).into_bytes());
        }
    }

    // write

    #[test]
    fn write_produces_what_to_string_produces() {
        for subject in subjects() {
            let mut out = Vec::new();

            write(&subject, &mut out).unwrap();

            assert_eq!(out, to_vec(&subject));
        }
    }

    #[test]
    fn write_produces_what_to_string_produces_at_every_group_boundary() {
        for len in 0..32 {
            let subject = Sexp::atom(vec![b'x'; len]);
            let mut out = Vec::new();

            write(&subject, &mut out).unwrap();

            assert_eq!(out, to_vec(&subject));
        }
    }

    #[test]
    fn write_reports_a_failure_of_the_writer() {
        struct Full;

        impl io::Write for Full {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::StorageFull))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        assert!(write(&Sexp::atom("data"), &mut Full).is_err());
    }

    #[test]
    fn write_does_not_recurse() {
        let deep = nested(DEEP);
        let mut out = Vec::new();

        write(&deep, &mut out).unwrap();

        assert_eq!(out.len(), encoded_len(&deep));

        dismantle(deep);
    }

    // encoded_len

    #[test]
    fn encoded_len_matches_what_is_written() {
        for len in 0..32 {
            let subject = Sexp::atom(vec![b'x'; len]);

            assert_eq!(encoded_len(&subject), to_string(&subject).len());
        }

        for subject in subjects() {
            assert_eq!(encoded_len(&subject), to_string(&subject).len());
        }
    }

    #[test]
    fn encoded_len_counts_the_braces() {
        // The empty list is two octets canonically, which is one whole group
        // of four characters.
        assert_eq!(encoded_len(&Sexp::list([])), 6);
    }
}
