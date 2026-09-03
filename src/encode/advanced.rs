//! The advanced representation ([RFC 9804], §6.4).
//!
//! §6.4 is the representation meant to be read by a person. §4 gives an octet
//! string five forms there, and this module chooses among three of them by
//! what the octets are.
//!
//! An octet string that qualifies as a token under §4.3 is written as one,
//! bare, which is the shortest and plainest form. An octet string that does
//! not qualify but reads as text is written as a quoted string (§4.2), with a
//! backslash escape for the two characters that would end or continue it and
//! for the seven control characters §4.2 names. Anything else is written in
//! hexadecimal (§4.4), which stays legible where an escaped binary string
//! would not. Reading as text here means every octet is either printable ASCII
//! or one of those seven control characters.
//!
//! §4.1 verbatim and §4.5 base 64 are never written. The first is what the
//! canonical representation is made of, and the second is harder to read than
//! hexadecimal at the sizes an advanced representation is meant for. Both are
//! read back without complaint by [`crate::decode`].
//!
//! No length is written before a quoted string or a hexadecimal string, though
//! §4.2 and §4.4 allow one, since §8 lets an application refuse lengths there.
//! Elements of a list are separated by one space, which §5 requires between
//! two tokens and permits everywhere else. Every character written is ASCII.
//!
//! This representation is not unique. The form each octet string takes is a
//! choice, so two S-expressions that are equal have equal advanced
//! representations here, but an S-expression read from elsewhere may have been
//! written another way. Hash and sign the canonical representation of §6.2
//! instead, which [`crate::encode::canonical`] writes.
//!
//! [RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html

use std::fmt;
use std::io;

use crate::syntax;
use crate::types::{Atom, Sexp};

/// The seven control characters §4.2 gives a one-letter escape.
const ESCAPED: &[(u8, char)] = &[
    (0x07, 'a'),
    (0x08, 'b'),
    (0x09, 't'),
    (0x0a, 'n'),
    (0x0b, 'v'),
    (0x0c, 'f'),
    (0x0d, 'r'),
];

/// Returns the advanced representation of the S-expression.
///
/// This is what [`Display`] writes, so `sexp.to_string()` returns the same
/// text.
///
/// # Examples
///
/// ```
/// use sexp::{Atom, Sexp, encode::advanced, sexp};
///
/// let cert = sexp!["issuer", Atom::new("bob").with_hint("text/plain")];
///
/// assert_eq!(advanced::to_string(&cert), "(issuer [text/plain]bob)");
/// assert_eq!(
///     advanced::to_string(&Sexp::atom("hello world")),
///     r#""hello world""#
/// );
/// assert_eq!(advanced::to_string(&Sexp::atom([0xff, 0x00])), "#ff00#");
/// ```
///
/// [`Display`]: std::fmt::Display
pub fn to_string(sexp: &Sexp) -> String {
    let mut out = String::new();

    // Writing into a String cannot fail, so there is no error to propagate.
    let _ = write_sexp(sexp, &mut out);

    out
}

/// Returns the advanced representation of the S-expression, as octets.
///
/// Every character written is ASCII, so this is [`to_string`] one octet per
/// character.
pub fn to_vec(sexp: &Sexp) -> Vec<u8> {
    to_string(sexp).into_bytes()
}

/// Writes the advanced representation of the S-expression to `out`.
///
/// The characters reach `out` as they are produced, so writing this way never
/// holds the whole representation in memory, as [`to_string`] does.
///
/// # Errors
///
/// Returns whatever error `out` returns, at the point it returns it, leaving
/// however much was written before that already written.
pub fn write(sexp: &Sexp, out: &mut impl io::Write) -> io::Result<()> {
    let mut sink = Sink {
        inner: out,
        error: None,
    };

    match write_sexp(sexp, &mut sink) {
        Ok(()) => Ok(()),
        // A formatter of this kind fails only where the sink failed, so the
        // error it kept is the one to report.
        Err(_) => Err(sink
            .error
            .unwrap_or_else(|| io::Error::other("advanced representation not written"))),
    }
}

/// Writes the advanced representation of the S-expression, as §6.4 gives it.
///
/// This is what [`crate::encode::advanced`] writes. Reach it through
/// [`to_string`] to name the representation at the call site.
impl fmt::Display for Sexp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_sexp(self, f)
    }
}

/// A place to put characters that is really a place to put octets.
///
/// [`std::fmt`] reports that a sink failed without saying how, so the error
/// the writer gave is kept here to be reported in its place.
struct Sink<W> {
    inner: W,
    error: Option<io::Error>,
}

impl<W: io::Write> fmt::Write for Sink<W> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.inner.write_all(text.as_bytes()).map_err(|error| {
            self.error = Some(error);
            fmt::Error
        })
    }
}

/// Writes one S-expression, without recursing into its lists.
fn write_sexp(sexp: &Sexp, out: &mut impl fmt::Write) -> fmt::Result {
    enum Step<'a> {
        Node(&'a Sexp),
        Separator,
        Close,
    }

    let mut stack = vec![Step::Node(sexp)];

    while let Some(step) = stack.pop() {
        match step {
            Step::Separator => out.write_char(' ')?,
            Step::Close => out.write_char(')')?,
            Step::Node(Sexp::Atom(atom)) => write_atom(atom, out)?,
            Step::Node(Sexp::List(items)) => {
                out.write_char('(')?;
                stack.push(Step::Close);

                for (index, item) in items.iter().enumerate().rev() {
                    stack.push(Step::Node(item));

                    if index > 0 {
                        stack.push(Step::Separator);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Writes one octet string, preceded by its display hint if it has one.
fn write_atom(atom: &Atom, out: &mut impl fmt::Write) -> fmt::Result {
    if let Some(hint) = atom.hint() {
        out.write_char('[')?;
        write_octet_string(hint, out)?;
        out.write_char(']')?;
    }

    write_octet_string(atom.data(), out)
}

/// Writes one octet string in the shortest form that suits its octets.
fn write_octet_string(octets: &[u8], out: &mut impl fmt::Write) -> fmt::Result {
    if syntax::qualifies_as_token(octets) {
        for &octet in octets {
            out.write_char(char::from(octet))?;
        }

        return Ok(());
    }

    if octets.iter().copied().all(is_text) {
        return write_quoted_string(octets, out);
    }

    write_hexadecimal(octets, out)
}

/// Writes one octet string as a quoted string (§4.2).
fn write_quoted_string(octets: &[u8], out: &mut impl fmt::Write) -> fmt::Result {
    out.write_char('"')?;

    for &octet in octets {
        match ESCAPED.iter().find(|(control, _)| *control == octet) {
            Some((_, escape)) => {
                out.write_char('\\')?;
                out.write_char(*escape)?;
            }
            None => {
                if octet == b'"' || octet == b'\\' {
                    out.write_char('\\')?;
                }

                out.write_char(char::from(octet))?;
            }
        }
    }

    out.write_char('"')
}

/// Writes one octet string in hexadecimal (§4.4).
fn write_hexadecimal(octets: &[u8], out: &mut impl fmt::Write) -> fmt::Result {
    out.write_char('#')?;

    for &octet in octets {
        write!(out, "{octet:02x}")?;
    }

    out.write_char('#')
}

/// Returns `true` if the octet may stand in a quoted string as itself or as a
/// one-letter escape.
fn is_text(octet: u8) -> bool {
    let printable = (0x20..=0x7e).contains(&octet);

    printable || ESCAPED.iter().any(|(control, _)| *control == octet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexp;

    /// A nesting deeper than any stack the tests may assume.
    const DEEP: usize = 100_000;

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

    /// Returns the advanced representation of one octet string alone.
    fn atom(octets: impl AsRef<[u8]>) -> String {
        to_string(&Sexp::atom(octets))
    }

    // to_string, octet strings as tokens

    #[test]
    fn to_string_writes_a_qualifying_octet_string_as_a_token() {
        assert_eq!(atom("issuer"), "issuer");
        assert_eq!(atom("text/plain"), "text/plain");
        assert_eq!(atom("a1"), "a1");
        assert_eq!(atom("-.foo_:*+="), "-.foo_:*+=");
    }

    #[test]
    fn to_string_writes_no_length_before_a_token() {
        assert!(!atom("issuer").starts_with(|c: char| c.is_ascii_digit()));
    }

    #[test]
    fn to_string_quotes_an_octet_string_that_begins_with_a_digit() {
        assert_eq!(atom("3abc"), r#""3abc""#);
        assert_eq!(atom("1"), r#""1""#);
    }

    // to_string, octet strings as quoted strings

    #[test]
    fn to_string_writes_text_as_a_quoted_string() {
        assert_eq!(atom("hello world"), r#""hello world""#);
        assert_eq!(atom("a(b"), r#""a(b""#);
    }

    #[test]
    fn to_string_writes_a_zero_length_octet_string_as_two_quotes() {
        assert_eq!(atom(""), r#""""#);
    }

    #[test]
    fn to_string_escapes_the_quote_and_the_backslash() {
        assert_eq!(atom(r#"a"b"#), r#""a\"b""#);
        assert_eq!(atom(r"a\b"), r#""a\\b""#);
    }

    #[test]
    fn to_string_escapes_the_seven_named_control_characters() {
        let controls: Vec<u8> = ESCAPED.iter().map(|(octet, _)| *octet).collect();

        assert_eq!(atom(controls), r#""\a\b\t\n\v\f\r""#);
    }

    #[test]
    fn to_string_writes_no_length_before_a_quoted_string() {
        assert!(atom("hello world").starts_with('"'));
    }

    // to_string, octet strings in hexadecimal

    #[test]
    fn to_string_writes_octets_that_are_not_text_in_hexadecimal() {
        assert_eq!(atom([0xff, 0x00]), "#ff00#");
        assert_eq!(atom([0x7f]), "#7f#");
        assert_eq!(atom(b"text\x80"), "#7465787480#");
    }

    #[test]
    fn to_string_writes_two_hexadecimal_digits_for_every_octet() {
        let encoded = atom([0x00, 0x01, 0x0f, 0x10, 0xff]);

        assert_eq!(encoded, "#00010f10ff#");
        assert_eq!(encoded.len(), 2 + 2 * 5);
    }

    #[test]
    fn to_string_writes_no_length_before_a_hexadecimal_string() {
        assert!(atom([0xff]).starts_with('#'));
    }

    #[test]
    fn to_string_never_writes_a_verbatim_or_base_64_octet_string() {
        for octets in [b"data".as_slice(), b"a b", &[0xff, 0x00]] {
            let encoded = atom(octets);

            assert!(!encoded.contains(':'));
            assert!(!encoded.contains('|'));
        }
    }

    // to_string, display hints

    #[test]
    fn to_string_writes_a_display_hint_in_brackets_before_the_data() {
        let hinted = Atom::new("bob").with_hint("text/plain");

        assert_eq!(to_string(&Sexp::from(hinted)), "[text/plain]bob");
    }

    #[test]
    fn to_string_chooses_the_form_of_a_hint_by_its_own_octets() {
        let hinted = Atom::new("bob").with_hint([0xff]);

        assert_eq!(to_string(&Sexp::from(hinted)), "[#ff#]bob");
    }

    #[test]
    fn to_string_writes_a_zero_length_display_hint() {
        let hinted = Atom::new("bob").with_hint("");

        assert_eq!(to_string(&Sexp::from(hinted)), r#"[""]bob"#);
    }

    #[test]
    fn to_string_puts_no_space_between_a_hint_and_its_data() {
        let hinted = Atom::new("bob").with_hint("hint");

        assert_eq!(to_string(&Sexp::from(hinted)), "[hint]bob");
    }

    // to_string, lists

    #[test]
    fn to_string_writes_a_list_in_parentheses() {
        assert_eq!(to_string(&sexp!["issuer", "bob"]), "(issuer bob)");
    }

    #[test]
    fn to_string_writes_the_empty_list() {
        assert_eq!(to_string(&Sexp::list([])), "()");
    }

    #[test]
    fn to_string_separates_elements_with_one_space() {
        assert_eq!(to_string(&sexp!["a", "b", "c"]), "(a b c)");
    }

    #[test]
    fn to_string_puts_no_space_inside_the_parentheses() {
        let encoded = to_string(&sexp!["a"]);

        assert_eq!(encoded, "(a)");
        assert!(!encoded.contains("( "));
        assert!(!encoded.contains(" )"));
    }

    #[test]
    fn to_string_writes_nested_lists() {
        let sexp = sexp![sexp![], sexp!["a", sexp!["b"]]];

        assert_eq!(to_string(&sexp), "(() (a (b)))");
    }

    #[test]
    fn to_string_writes_only_ascii() {
        let sexp = sexp!["a b", Sexp::atom([0xff]), "c"];

        assert!(to_string(&sexp).is_ascii());
    }

    #[test]
    fn to_string_does_not_recurse() {
        let deep = nested(DEEP);

        assert_eq!(to_string(&deep).len(), 2 * DEEP + "data".len());

        dismantle(deep);
    }

    // Display

    #[test]
    fn display_writes_what_to_string_writes() {
        let subjects = [
            Sexp::atom("data"),
            Sexp::atom([0xff]),
            Sexp::from(Atom::new("bob").with_hint("hint")),
            sexp!["a", sexp!["b"]],
            Sexp::list([]),
        ];

        for subject in subjects {
            assert_eq!(subject.to_string(), to_string(&subject));
            assert_eq!(format!("{subject}"), to_string(&subject));
        }
    }

    // to_vec, write

    #[test]
    fn to_vec_is_to_string_as_octets() {
        let sexp = sexp!["a b", Sexp::atom([0xff])];

        assert_eq!(to_vec(&sexp), to_string(&sexp).into_bytes());
    }

    #[test]
    fn write_produces_what_to_string_produces() {
        let subjects = [
            Sexp::atom(""),
            Sexp::atom("data"),
            Sexp::atom([0x00, 0xff]),
            Sexp::from(Atom::new("bob").with_hint("hint")),
            sexp![sexp![], "a b"],
        ];

        for subject in subjects {
            let mut out = Vec::new();

            write(&subject, &mut out).unwrap();

            assert_eq!(out, to_vec(&subject));
        }
    }

    #[test]
    fn write_reports_the_failure_the_writer_reported() {
        struct Full;

        impl io::Write for Full {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::StorageFull))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let failure = write(&Sexp::atom("data"), &mut Full).unwrap_err();

        assert_eq!(failure.kind(), io::ErrorKind::StorageFull);
    }

    #[test]
    fn write_does_not_recurse() {
        let deep = nested(DEEP);
        let mut out = Vec::new();

        write(&deep, &mut out).unwrap();

        assert_eq!(out.len(), 2 * DEEP + "data".len());

        dismantle(deep);
    }

    // is_text

    #[test]
    fn is_text_admits_printable_ascii_and_the_seven_escapes() {
        for octet in 0..=u8::MAX {
            let printable = (0x20..=0x7e).contains(&octet);
            let escaped = (0x07..=0x0d).contains(&octet);

            assert_eq!(is_text(octet), printable || escaped, "octet {octet:#04x}");
        }
    }
}
