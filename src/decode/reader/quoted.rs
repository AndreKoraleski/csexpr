//! Reading an octet string written between quotes (RFC 9804, §4.2).
//!
//! §4.2 writes text as itself between two `"` characters, and everything else
//! as a backslash escape. Seven control characters have an escape named by a
//! letter, four characters that would otherwise end or confuse the string have
//! one named by the character, and any octet at all may be named by three
//! octal digits or by `x` and two hexadecimal digits. A backslash before the
//! end of a line stands for nothing, and continues the string on the next.
//!
//! A length may precede the opening quote, stating how many octets the string
//! decodes to. Where one is given it has to agree with what was read.

use crate::decode::error::{Error, ErrorKind};
use crate::decode::reader::Reader;
use crate::syntax;

impl Reader<'_> {
    /// Reads an octet string given between quotes, escapes and all (§4.2).
    pub(super) fn quoted_string(
        &mut self,
        start: usize,
        declared: Option<usize>,
    ) -> Result<Vec<u8>, Error> {
        self.offset += 1;

        let mut octets = Vec::new();

        loop {
            let octet = self.peek_required()?;
            self.offset += 1;

            match octet {
                b'"' => break,
                b'\\' => {
                    if let Some(escaped) = self.escape()? {
                        octets.push(escaped);
                    }
                }
                // §4.2 admits printable characters as themselves, and leaves
                // every other octet to an escape.
                0x20..=0x7e => octets.push(octet),
                _ => return Err(Error::new(ErrorKind::UnexpectedOctet, self.offset - 1)),
            }
        }

        self.check_declared(declared, octets.len(), start)?;

        Ok(octets)
    }

    /// Reads what follows a backslash in a quoted string (§4.2).
    ///
    /// Returns nothing where the escape is one of the four that continue a
    /// line, which stand for no octet at all.
    fn escape(&mut self) -> Result<Option<u8>, Error> {
        let start = self.offset - 1;
        let octet = self.peek_required()?;

        self.offset += 1;

        let escaped = match octet {
            b'a' => 0x07,
            b'b' => 0x08,
            b't' => 0x09,
            b'n' => 0x0a,
            b'v' => 0x0b,
            b'f' => 0x0c,
            b'r' => 0x0d,
            b'"' => b'"',
            b'\'' => b'\'',
            b'?' => b'?',
            b'\\' => b'\\',
            b'x' => self.hexadecimal_escape(start)?,
            b'0'..=b'7' => self.octal_escape(octet, start)?,
            b'\r' | b'\n' => {
                // A carriage return and a line feed, in either order, continue
                // the line together rather than one after the other.
                let paired = if octet == b'\r' { b'\n' } else { b'\r' };

                if self.peek() == Some(paired) {
                    self.offset += 1;
                }

                return Ok(None);
            }
            _ => return Err(Error::new(ErrorKind::InvalidEscape, start)),
        };

        Ok(Some(escaped))
    }

    /// Reads the two digits of an escape that names an octet in hexadecimal.
    fn hexadecimal_escape(&mut self, start: usize) -> Result<u8, Error> {
        let mut value = 0u8;

        for _ in 0..2 {
            let digit = syntax::hex_value(self.peek_required()?)
                .ok_or_else(|| Error::new(ErrorKind::InvalidEscape, start))?;

            value = (value << 4) | digit;
            self.offset += 1;
        }

        Ok(value)
    }

    /// Reads the rest of an escape that names an octet in octal, given its
    /// first digit.
    fn octal_escape(&mut self, first: u8, start: usize) -> Result<u8, Error> {
        let mut value = u32::from(first - b'0');

        for _ in 0..2 {
            let digit = self.peek_required()?;

            if !(b'0'..=b'7').contains(&digit) {
                return Err(Error::new(ErrorKind::InvalidEscape, start));
            }

            value = value * 8 + u32::from(digit - b'0');
            self.offset += 1;
        }

        // Three octal digits reach 511, and an octet stops at 255.
        u8::try_from(value).map_err(|_| Error::new(ErrorKind::InvalidEscape, start))
    }
}

#[cfg(test)]
mod tests {
    use crate::decode::{ErrorKind, parse};
    use crate::types::Sexp;

    /// Asserts that the input is refused for the given reason, at the given
    /// offset.
    fn assert_refused(input: &[u8], kind: ErrorKind, offset: usize) {
        let error = parse(input).unwrap_err();
        let seen = String::from_utf8_lossy(input).into_owned();

        assert_eq!(error.kind(), kind, "parsing {seen}");
        assert_eq!(error.offset(), offset, "parsing {seen}");
    }

    #[test]
    fn reads_a_quoted_string() {
        assert_eq!(
            parse(br#""hello world""#).unwrap(),
            Sexp::atom("hello world")
        );
        assert_eq!(parse(br#""""#).unwrap(), Sexp::atom(""));
    }

    #[test]
    fn reads_the_escapes_a_quoted_string_names_by_letter() {
        let expected = Sexp::atom([0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d]);

        assert_eq!(parse(br#""\a\b\t\n\v\f\r""#).unwrap(), expected);
    }

    #[test]
    fn reads_the_escapes_a_quoted_string_names_by_character() {
        assert_eq!(parse(br#""\"\'\?\\""#).unwrap(), Sexp::atom(r#""'?\"#));
    }

    #[test]
    fn reads_an_escape_that_names_an_octet_in_octal() {
        assert_eq!(parse(br#""\101""#).unwrap(), Sexp::atom("A"));
        assert_eq!(parse(br#""\000""#).unwrap(), Sexp::atom([0x00]));
        assert_eq!(parse(br#""\377""#).unwrap(), Sexp::atom([0xff]));
    }

    #[test]
    fn reads_an_escape_that_names_an_octet_in_hexadecimal() {
        assert_eq!(parse(br#""\x41""#).unwrap(), Sexp::atom("A"));
        assert_eq!(parse(br#""\x00\xFF""#).unwrap(), Sexp::atom([0x00, 0xff]));
    }

    #[test]
    fn reads_an_escape_that_continues_a_line() {
        assert_eq!(parse(b"\"a\\\nb\"").unwrap(), Sexp::atom("ab"));
        assert_eq!(parse(b"\"a\\\rb\"").unwrap(), Sexp::atom("ab"));
        assert_eq!(parse(b"\"a\\\r\nb\"").unwrap(), Sexp::atom("ab"));
        assert_eq!(parse(b"\"a\\\n\rb\"").unwrap(), Sexp::atom("ab"));
    }

    #[test]
    fn refuses_an_escape_that_is_not_defined() {
        assert_refused(br#""\q""#, ErrorKind::InvalidEscape, 1);
        assert_refused(br#""\18""#, ErrorKind::InvalidEscape, 1);
        assert_refused(br#""\xg0""#, ErrorKind::InvalidEscape, 1);
    }

    #[test]
    fn refuses_an_octal_escape_past_what_an_octet_holds() {
        assert_refused(br#""\400""#, ErrorKind::InvalidEscape, 1);
        assert_refused(br#""\777""#, ErrorKind::InvalidEscape, 1);
    }

    #[test]
    fn refuses_an_escape_that_is_cut_short() {
        assert_refused(br#""\x4""#, ErrorKind::InvalidEscape, 1);
        assert_refused(br#""\10""#, ErrorKind::InvalidEscape, 1);
        assert_refused(b"\"\\", ErrorKind::UnexpectedEnd, 2);
    }

    #[test]
    fn refuses_an_octet_that_cannot_stand_in_a_quoted_string() {
        assert_refused(b"\"a\nb\"", ErrorKind::UnexpectedOctet, 2);
        assert_refused(b"\"\x00\"", ErrorKind::UnexpectedOctet, 1);
        assert_refused(b"\"\xff\"", ErrorKind::UnexpectedOctet, 1);
    }

    #[test]
    fn refuses_a_quoted_string_that_does_not_close() {
        assert_refused(br#""abc"#, ErrorKind::UnexpectedEnd, 4);
    }
}
