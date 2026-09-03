//! The canonical representation ([RFC 9804], §6.2).
//!
//! §6.2 writes every octet string verbatim, as its length in decimal, a colon,
//! and the octets themselves, and it writes every list as its elements between
//! parentheses with nothing separating them. A display hint is written the
//! same way, in square brackets, immediately before the octet string it
//! qualifies.
//!
//! One S-expression has exactly one canonical representation, which is why
//! §6.2 makes it the form to hash or sign. Nothing about it is negotiable, so
//! nothing here is configurable. Two S-expressions that compare equal by the
//! criterion §4.7 recommends, which [`Sexp`] implements, have equal canonical
//! representations, and two that differ have different ones. An atom carrying
//! no display hint and an atom carrying the hint an application would supply
//! by default are different S-expressions under §4.7 and stay different here,
//! so [`Atom::effective_hint`] has no part in this.
//!
//! The output is a sequence of octets rather than text, since §4.1 writes the
//! data of an octet string exactly as it stands.
//!
//! [RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html
//! [`Atom::effective_hint`]: crate::types::Atom::effective_hint

use std::io;

use crate::types::{Atom, Sexp};

/// Room for the decimal digits of any `usize`, on any width it may have.
const DECIMAL_DIGITS: usize = usize::BITS as usize / 3 + 2;

/// Returns the canonical representation of the S-expression.
///
/// # Examples
///
/// ```
/// use sexp::{Atom, Sexp, encode::canonical, sexp};
///
/// let cert = sexp!["issuer", Atom::new("bob").with_hint("text/plain")];
///
/// assert_eq!(canonical::to_vec(&cert), b"(6:issuer[10:text/plain]3:bob)");
/// ```
pub fn to_vec(sexp: &Sexp) -> Vec<u8> {
    let mut out = Vec::with_capacity(encoded_len(sexp));

    // Writing into a Vec fails only if it cannot grow, which aborts rather
    // than returning, so there is no error here to propagate.
    let _ = write(sexp, &mut out);

    out
}

/// Writes the canonical representation of the S-expression to `out`.
///
/// The octets reach `out` as they are produced, so a writer that buffers is
/// worth using for anything larger than a small S-expression.
///
/// # Errors
///
/// Returns whatever error `out` returns, at the point it returns it, leaving
/// however much was written before that already written.
pub fn write(sexp: &Sexp, out: &mut impl io::Write) -> io::Result<()> {
    enum Step<'a> {
        Node(&'a Sexp),
        Close,
    }

    let mut stack = vec![Step::Node(sexp)];

    while let Some(step) = stack.pop() {
        match step {
            Step::Close => out.write_all(b")")?,
            Step::Node(Sexp::Atom(atom)) => write_atom(atom, out)?,
            Step::Node(Sexp::List(items)) => {
                out.write_all(b"(")?;
                stack.push(Step::Close);
                stack.extend(items.iter().rev().map(Step::Node));
            }
        }
    }

    Ok(())
}

/// Returns how many octets the canonical representation occupies.
///
/// This is what [`to_vec`] allocates, and it is exact rather than an estimate,
/// so an application that caps the size of what it emits can measure before
/// writing.
pub fn encoded_len(sexp: &Sexp) -> usize {
    let mut len = 0usize;

    for node in sexp.preorder() {
        len = len.saturating_add(match node {
            // One octet for each parenthesis.
            Sexp::List(_) => 2,
            Sexp::Atom(atom) => atom_len(atom),
        });
    }

    len
}

/// Writes one octet string, preceded by its display hint if it has one.
fn write_atom(atom: &Atom, out: &mut impl io::Write) -> io::Result<()> {
    if let Some(hint) = atom.hint() {
        out.write_all(b"[")?;
        write_octet_string(hint, out)?;
        out.write_all(b"]")?;
    }

    write_octet_string(atom.data(), out)
}

/// Writes one octet string verbatim, as §4.1 gives it.
fn write_octet_string(octets: &[u8], out: &mut impl io::Write) -> io::Result<()> {
    write_decimal(octets.len(), out)?;
    out.write_all(b":")?;
    out.write_all(octets)
}

/// Writes a length in decimal, with no leading zero, as §7.2 requires.
fn write_decimal(value: usize, out: &mut impl io::Write) -> io::Result<()> {
    let mut digits = [0u8; DECIMAL_DIGITS];
    let mut start = digits.len();
    let mut rest = value;

    loop {
        start -= 1;
        digits[start] = b'0' + (rest % 10) as u8;
        rest /= 10;

        if rest == 0 {
            break;
        }
    }

    out.write_all(&digits[start..])
}

/// Returns how many octets one atom occupies, hint included.
fn atom_len(atom: &Atom) -> usize {
    let data = octet_string_len(atom.data());

    match atom.hint() {
        // Two octets for the brackets around the hint.
        Some(hint) => data
            .saturating_add(octet_string_len(hint))
            .saturating_add(2),
        None => data,
    }
}

/// Returns how many octets one verbatim octet string occupies.
fn octet_string_len(octets: &[u8]) -> usize {
    // One octet for the colon.
    decimal_len(octets.len())
        .saturating_add(1)
        .saturating_add(octets.len())
}

/// Returns how many digits a length occupies in decimal.
fn decimal_len(value: usize) -> usize {
    let mut digits = 1;
    let mut rest = value;

    while rest >= 10 {
        rest /= 10;
        digits += 1;
    }

    digits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexp;

    const OCTETS: &[u8] = &[0x00, 0xff, 0x1b, 0x7f];

    /// A nesting deeper than any stack the tests may assume.
    const DEEP: usize = 100_000;

    /// A writer that accepts a fixed number of octets and then fails.
    struct Failing {
        written: Vec<u8>,
        remaining: usize,
    }

    impl Failing {
        fn new(remaining: usize) -> Self {
            Self {
                written: Vec::new(),
                remaining,
            }
        }
    }

    impl io::Write for Failing {
        fn write(&mut self, octets: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::from(io::ErrorKind::StorageFull));
            }

            let accepted = octets.len().min(self.remaining);
            self.written.extend_from_slice(&octets[..accepted]);
            self.remaining -= accepted;

            Ok(accepted)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
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

    /// Every shape the representation has to distinguish.
    fn corpus() -> Vec<Sexp> {
        vec![
            Sexp::atom(""),
            Sexp::atom("data"),
            Sexp::atom(OCTETS),
            Sexp::atom(vec![b'x'; 1000]),
            Sexp::from(Atom::new("data").with_hint("hint")),
            Sexp::from(Atom::new("").with_hint("")),
            Sexp::from(Atom::new(OCTETS).with_hint(OCTETS)),
            Sexp::list([]),
            sexp!["a", "b"],
            sexp![sexp![], sexp![sexp!["a"]]],
            nested(20),
        ]
    }

    // to_vec

    #[test]
    fn to_vec_writes_an_octet_string_verbatim() {
        assert_eq!(to_vec(&Sexp::atom("data")), b"4:data");
    }

    #[test]
    fn to_vec_writes_a_zero_length_octet_string() {
        assert_eq!(to_vec(&Sexp::atom("")), b"0:");
    }

    #[test]
    fn to_vec_writes_arbitrary_octets_unaltered() {
        let expected = [b"4:".as_slice(), OCTETS].concat();

        assert_eq!(to_vec(&Sexp::atom(OCTETS)), expected);
    }

    #[test]
    fn to_vec_writes_octets_that_resemble_syntax_unaltered() {
        let data: &[u8] = b"()[]{}|#\"3:abc";

        assert_eq!(
            to_vec(&Sexp::atom(data)),
            [b"14:".as_slice(), data].concat()
        );
    }

    #[test]
    fn to_vec_writes_a_display_hint_before_the_data() {
        let atom = Atom::new("data").with_hint("hint");

        assert_eq!(to_vec(&Sexp::from(atom)), b"[4:hint]4:data");
    }

    #[test]
    fn to_vec_writes_a_zero_length_display_hint() {
        let atom = Atom::new("data").with_hint("");

        assert_eq!(to_vec(&Sexp::from(atom)), b"[0:]4:data");
    }

    #[test]
    fn to_vec_writes_a_list_in_parentheses() {
        assert_eq!(to_vec(&sexp!["issuer", "bob"]), b"(6:issuer3:bob)");
    }

    #[test]
    fn to_vec_writes_the_empty_list() {
        assert_eq!(to_vec(&Sexp::list([])), b"()");
    }

    #[test]
    fn to_vec_separates_elements_with_nothing() {
        let encoded = to_vec(&sexp!["a", "b", "c"]);

        assert_eq!(encoded, b"(1:a1:b1:c)");
        assert!(!encoded.iter().any(u8::is_ascii_whitespace));
    }

    #[test]
    fn to_vec_writes_nested_lists() {
        let sexp = sexp![sexp![], sexp!["a", sexp!["b"]]];

        assert_eq!(to_vec(&sexp), b"(()(1:a(1:b)))");
    }

    #[test]
    fn to_vec_writes_lengths_without_a_leading_zero() {
        let sexp = Sexp::atom(vec![b'x'; 100]);

        assert!(to_vec(&sexp).starts_with(b"100:"));
    }

    #[test]
    fn to_vec_writes_lengths_of_more_than_one_digit() {
        for len in [9usize, 10, 11, 99, 100, 1000] {
            let encoded = to_vec(&Sexp::atom(vec![b'x'; len]));
            let expected = format!("{len}:");

            assert!(encoded.starts_with(expected.as_bytes()));
            assert_eq!(encoded.len(), expected.len() + len);
        }
    }

    #[test]
    fn to_vec_distinguishes_an_absent_hint_from_an_explicit_default() {
        let hinted = Atom::new("data").with_hint(crate::types::DEFAULT_HINT);

        assert_ne!(to_vec(&Sexp::from(hinted)), to_vec(&Sexp::atom("data")));
    }

    #[test]
    fn to_vec_distinguishes_an_atom_from_a_list_holding_it() {
        assert_ne!(to_vec(&Sexp::atom("a")), to_vec(&sexp!["a"]));
    }

    #[test]
    fn to_vec_agrees_with_equality() {
        for left in corpus() {
            for right in corpus() {
                assert_eq!(left == right, to_vec(&left) == to_vec(&right));
            }
        }
    }

    #[test]
    fn to_vec_does_not_recurse() {
        let deep = nested(DEEP);

        assert_eq!(to_vec(&deep).len(), 2 * DEEP + "4:data".len());

        dismantle(deep);
    }

    // write

    #[test]
    fn write_produces_what_to_vec_produces() {
        for sexp in corpus() {
            let mut out = Vec::new();

            write(&sexp, &mut out).unwrap();

            assert_eq!(out, to_vec(&sexp));
        }
    }

    #[test]
    fn write_reports_a_failure_of_the_writer() {
        let mut out = Failing::new(0);

        assert!(write(&Sexp::atom("data"), &mut out).is_err());
    }

    #[test]
    fn write_leaves_what_it_wrote_before_a_failure() {
        let mut out = Failing::new(3);

        assert!(write(&sexp!["data"], &mut out).is_err());
        assert_eq!(out.written, b"(4:");
    }

    // encoded_len

    #[test]
    fn encoded_len_matches_what_is_written() {
        for sexp in corpus() {
            assert_eq!(encoded_len(&sexp), to_vec(&sexp).len());
        }
    }

    #[test]
    fn encoded_len_counts_the_empty_list_and_the_empty_atom() {
        assert_eq!(encoded_len(&Sexp::list([])), 2);
        assert_eq!(encoded_len(&Sexp::atom("")), 2);
    }

    #[test]
    fn encoded_len_counts_the_display_hint() {
        let hinted = Sexp::from(Atom::new("data").with_hint("hint"));

        assert_eq!(encoded_len(&hinted), encoded_len(&Sexp::atom("data")) + 8);
    }

    #[test]
    fn encoded_len_does_not_recurse() {
        let deep = nested(DEEP);

        assert_eq!(encoded_len(&deep), 2 * DEEP + "4:data".len());

        dismantle(deep);
    }

    // decimal_len, write_decimal

    #[test]
    fn decimal_len_counts_the_digits_of_a_length() {
        assert_eq!(decimal_len(0), 1);
        assert_eq!(decimal_len(9), 1);
        assert_eq!(decimal_len(10), 2);
        assert_eq!(decimal_len(99), 2);
        assert_eq!(decimal_len(100), 3);
        assert_eq!(decimal_len(usize::MAX), usize::MAX.to_string().len());
    }

    #[test]
    fn write_decimal_agrees_with_decimal_len() {
        for value in [0usize, 1, 9, 10, 255, 1000, usize::MAX] {
            let mut out = Vec::new();

            write_decimal(value, &mut out).unwrap();

            assert_eq!(out, value.to_string().as_bytes());
            assert_eq!(out.len(), decimal_len(value));
        }
    }
}
