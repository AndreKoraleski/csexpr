//! Reading one octet string, in any of the forms §4 gives it.
//!
//! §4 writes an octet string five ways. Verbatim (§4.1) is a length, a colon,
//! and the octets, which is the only form the canonical representation uses. A
//! quoted string (§4.2) is text between quotes, with backslash escapes. A
//! token (§4.3) is the octets standing bare. Hexadecimal (§4.4) is two digits
//! per octet between two `#` characters, and base 64 (§4.5) is the encoding of
//! RFC 4648 between two `|` characters. The last four may each be preceded by
//! the length they decode to, and all four belong to the advanced
//! representation alone.
//!
//! Which form is in front of the reader is settled by its first octet, except
//! that a digit begins either a verbatim octet string or the length before one
//! of the other three, which the octet after the digits settles.

use crate::decode::error::{Error, ErrorKind};
use crate::decode::reader::Reader;
use crate::{base64, syntax};

impl Reader<'_> {
    /// Reads one octet string, in whichever of the forms of §4 it is written.
    pub(super) fn simple_string(&mut self) -> Result<Vec<u8>, Error> {
        let start = self.offset;
        let octets = match self.peek_required()? {
            octet if octet.is_ascii_digit() => {
                let len = self.decimal()?;

                match self.peek_required()? {
                    b':' => {
                        self.offset += 1;
                        self.verbatim(len)?
                    }
                    b'"' => {
                        self.require_advanced(start)?;
                        self.require_lengths(start)?;
                        self.quoted_string(start, Some(len))?
                    }
                    b'#' => {
                        self.require_advanced(start)?;
                        self.require_hexadecimal(start)?;
                        self.require_lengths(start)?;
                        self.hexadecimal(start, Some(len))?
                    }
                    b'|' => {
                        self.require_advanced(start)?;
                        self.require_base64(start)?;
                        self.require_lengths(start)?;
                        self.base64_string(start, Some(len))?
                    }
                    _ => return Err(self.error(ErrorKind::UnexpectedOctet)),
                }
            }
            b'"' => {
                self.require_advanced(start)?;
                self.quoted_string(start, None)?
            }
            b'#' => {
                self.require_advanced(start)?;
                self.require_hexadecimal(start)?;
                self.hexadecimal(start, None)?
            }
            b'|' => {
                self.require_advanced(start)?;
                self.require_base64(start)?;
                self.base64_string(start, None)?
            }
            octet if syntax::is_token_start(octet) => {
                self.require_advanced(start)?;
                self.token()
            }
            _ => return Err(self.error(ErrorKind::UnexpectedOctet)),
        };

        self.check_octet_string(&octets, start)?;

        Ok(octets)
    }

    /// Reads an octet string given as its length, a colon, and its octets
    /// (§4.1).
    fn verbatim(&mut self, len: usize) -> Result<Vec<u8>, Error> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| self.error(ErrorKind::LengthOverflow))?;

        // Nothing is taken on the strength of the length alone, so a length
        // that overstates what follows costs a comparison and no memory.
        let octets = self
            .input
            .get(self.offset..end)
            .ok_or_else(|| Error::new(ErrorKind::UnexpectedEnd, self.input.len()))?;

        self.offset = end;

        Ok(octets.to_vec())
    }

    /// Reads an octet string given as hexadecimal digits between two `#`
    /// characters (§4.4).
    fn hexadecimal(&mut self, start: usize, declared: Option<usize>) -> Result<Vec<u8>, Error> {
        self.offset += 1;

        let mut octets = Vec::new();
        let mut high = None;

        loop {
            let octet = self.peek_required()?;
            self.offset += 1;

            if octet == b'#' {
                break;
            }

            if syntax::is_whitespace(octet) {
                continue;
            }

            let value = syntax::hex_value(octet)
                .ok_or_else(|| Error::new(ErrorKind::InvalidHexDigit, self.offset - 1))?;

            match high.take() {
                Some(first) => octets.push((first << 4) | value),
                None => high = Some(value),
            }
        }

        if high.is_some() {
            return Err(Error::new(ErrorKind::OddHexDigits, start));
        }

        self.check_declared(declared, octets.len(), start)?;

        Ok(octets)
    }

    /// Reads an octet string given in base 64 between two `|` characters
    /// (§4.5).
    fn base64_string(&mut self, start: usize, declared: Option<usize>) -> Result<Vec<u8>, Error> {
        self.offset += 1;

        let end = self.find(b'|')?;
        let octets = base64::decode(&self.input[self.offset..end]).map_err(|offset| {
            Error::new(ErrorKind::InvalidBase64, self.offset.saturating_add(offset))
        })?;

        self.offset = end + 1;
        self.check_declared(declared, octets.len(), start)?;

        Ok(octets)
    }

    /// Reads an octet string given as itself (§4.3).
    fn token(&mut self) -> Vec<u8> {
        let start = self.offset;

        while let Some(octet) = self.peek() {
            if !syntax::is_token(octet) {
                break;
            }

            self.offset += 1;
        }

        self.input[start..self.offset].to_vec()
    }

    /// Reads a length, which §7 writes in decimal with no leading zero.
    fn decimal(&mut self) -> Result<usize, Error> {
        let start = self.offset;
        let mut value = 0usize;
        let mut digits = 0usize;

        while let Some(octet) = self.peek() {
            if !octet.is_ascii_digit() {
                break;
            }

            if digits == 1 && value == 0 {
                return Err(Error::new(ErrorKind::LengthLeadingZero, start));
            }

            value = value
                .checked_mul(10)
                .and_then(|shifted| shifted.checked_add(usize::from(octet - b'0')))
                .ok_or_else(|| Error::new(ErrorKind::LengthOverflow, start))?;

            digits += 1;
            self.offset += 1;
        }

        Ok(value)
    }

    /// Reports an octet string the parser was told to refuse.
    fn check_octet_string(&self, octets: &[u8], start: usize) -> Result<(), Error> {
        if octets.is_empty() && !self.parser.empty_atoms {
            return Err(Error::new(ErrorKind::EmptyAtomNotAllowed, start));
        }

        if octets.len() > self.parser.max_atom_len {
            return Err(Error::new(ErrorKind::AtomTooLong, start));
        }

        Ok(())
    }

    /// Reports a construct of the advanced representation, where the parser
    /// accepts only the canonical one.
    fn require_advanced(&self, start: usize) -> Result<(), Error> {
        match self.parser.advanced {
            true => Ok(()),
            false => Err(Error::new(ErrorKind::AdvancedNotAllowed, start)),
        }
    }

    /// Reports a hexadecimal octet string the parser was told to refuse.
    fn require_hexadecimal(&self, start: usize) -> Result<(), Error> {
        match self.parser.hexadecimal {
            true => Ok(()),
            false => Err(Error::new(ErrorKind::HexadecimalNotAllowed, start)),
        }
    }

    /// Reports a base-64 octet string the parser was told to refuse.
    fn require_base64(&self, start: usize) -> Result<(), Error> {
        match self.parser.base64 {
            true => Ok(()),
            false => Err(Error::new(ErrorKind::Base64NotAllowed, start)),
        }
    }

    /// Reports a length the parser was told to refuse.
    fn require_lengths(&self, start: usize) -> Result<(), Error> {
        match self.parser.lengths {
            true => Ok(()),
            false => Err(Error::new(ErrorKind::LengthNotAllowed, start)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::decode::{ErrorKind, Parser, parse};
    use crate::types::Sexp;

    /// Asserts that the input is refused for the given reason, at the given
    /// offset.
    fn assert_refused(input: &[u8], kind: ErrorKind, offset: usize) {
        let error = parse(input).unwrap_err();
        let seen = String::from_utf8_lossy(input).into_owned();

        assert_eq!(error.kind(), kind, "parsing {seen}");
        assert_eq!(error.offset(), offset, "parsing {seen}");
    }

    // Verbatim (§4.1)

    #[test]
    fn reads_a_verbatim_octet_string() {
        assert_eq!(parse(b"4:data").unwrap(), Sexp::atom("data"));
    }

    #[test]
    fn reads_a_zero_length_verbatim_octet_string() {
        assert_eq!(parse(b"0:").unwrap(), Sexp::atom(""));
    }

    #[test]
    fn reads_octets_of_every_value_verbatim() {
        let octets: Vec<u8> = (0..=255).collect();
        let mut input = b"256:".to_vec();

        input.extend_from_slice(&octets);

        assert_eq!(parse(&input).unwrap(), Sexp::atom(octets));
    }

    #[test]
    fn reads_octets_that_resemble_syntax_verbatim() {
        assert_eq!(parse(b"5:(a b)").unwrap(), Sexp::atom("(a b)"));
        assert_eq!(parse(b"2:3:").unwrap(), Sexp::atom("3:"));
    }

    #[test]
    fn refuses_a_verbatim_octet_string_that_is_cut_short() {
        assert_refused(b"4:dat", ErrorKind::UnexpectedEnd, 5);
        assert_refused(b"4:", ErrorKind::UnexpectedEnd, 2);
    }

    #[test]
    fn refuses_a_length_that_overstates_what_follows() {
        // Nothing is taken on the strength of the length, so a length far past
        // the end costs no memory.
        assert_refused(b"999999999999:a", ErrorKind::UnexpectedEnd, 14);
    }

    // Lengths (§7)

    #[test]
    fn reads_a_length_that_is_a_single_zero() {
        assert_eq!(parse(b"0:").unwrap(), Sexp::atom(""));
    }

    #[test]
    fn reads_a_length_of_more_than_one_digit() {
        let data = vec![b'x'; 100];
        let mut input = b"100:".to_vec();

        input.extend_from_slice(&data);

        assert_eq!(parse(&input).unwrap(), Sexp::atom(data));
    }

    #[test]
    fn refuses_a_length_with_a_leading_zero() {
        assert_refused(b"04:data", ErrorKind::LengthLeadingZero, 0);
        assert_refused(b"(1:a 01:b)", ErrorKind::LengthLeadingZero, 5);
    }

    #[test]
    fn refuses_a_length_too_large_to_represent() {
        let input = format!("{}0:", usize::MAX);

        assert_refused(input.as_bytes(), ErrorKind::LengthOverflow, 0);
    }

    #[test]
    fn refuses_a_length_followed_by_no_octet_string() {
        assert_refused(b"4", ErrorKind::UnexpectedEnd, 1);
        assert_refused(b"4x", ErrorKind::UnexpectedOctet, 1);
    }

    #[test]
    fn reads_a_length_before_a_quoted_hexadecimal_or_base_64_string() {
        assert_eq!(parse(br#"3"abc""#).unwrap(), Sexp::atom("abc"));
        assert_eq!(parse(b"3#616263#").unwrap(), Sexp::atom("abc"));
        assert_eq!(parse(b"3|YWJj|").unwrap(), Sexp::atom("abc"));
    }

    #[test]
    fn refuses_a_length_that_disagrees_with_what_it_counts() {
        assert_refused(br#"2"abc""#, ErrorKind::LengthMismatch, 0);
        assert_refused(b"2#616263#", ErrorKind::LengthMismatch, 0);
        assert_refused(b"2|YWJj|", ErrorKind::LengthMismatch, 0);
    }

    #[test]
    fn refuses_a_length_where_the_parser_allows_none() {
        let parser = Parser::new().allow_lengths(false);

        for input in [br#"3"abc""#.as_slice(), b"3#616263#", b"3|YWJj|"] {
            let error = parser.parse(input).unwrap_err();

            assert_eq!(
                error.kind(),
                ErrorKind::LengthNotAllowed,
                "parsing {input:?}"
            );
            assert_eq!(error.offset(), 0);
        }

        assert_eq!(parser.parse(b"3:abc").unwrap(), Sexp::atom("abc"));
    }

    // Tokens (§4.3)

    #[test]
    fn reads_a_token() {
        assert_eq!(parse(b"issuer").unwrap(), Sexp::atom("issuer"));
        assert_eq!(parse(b"text/plain").unwrap(), Sexp::atom("text/plain"));
        assert_eq!(parse(b"-.a_:*+=1").unwrap(), Sexp::atom("-.a_:*+=1"));
    }

    #[test]
    fn reads_a_token_up_to_what_cannot_be_in_one() {
        assert_eq!(parse(b"(ab cd)").unwrap().get(0), Some(&Sexp::atom("ab")));
        assert_eq!(parse(b"(ab)").unwrap().get(0), Some(&Sexp::atom("ab")));
    }

    // Quoted strings (§4.2)

    // Hexadecimal (§4.4)

    #[test]
    fn reads_a_hexadecimal_octet_string() {
        assert_eq!(parse(b"#616263#").unwrap(), Sexp::atom("abc"));
        assert_eq!(parse(b"#00FF#").unwrap(), Sexp::atom([0x00, 0xff]));
        assert_eq!(parse(b"##").unwrap(), Sexp::atom(""));
    }

    #[test]
    fn reads_hexadecimal_in_either_case() {
        assert_eq!(parse(b"#abcdef#").unwrap(), parse(b"#ABCDEF#").unwrap());
    }

    #[test]
    fn reads_whitespace_inside_a_hexadecimal_octet_string() {
        assert_eq!(parse(b"#61 62\n63#").unwrap(), Sexp::atom("abc"));
        assert_eq!(parse(b"# #").unwrap(), Sexp::atom(""));
    }

    #[test]
    fn refuses_hexadecimal_that_is_not_hexadecimal() {
        assert_refused(b"#6g#", ErrorKind::InvalidHexDigit, 2);
        assert_refused(b"#61", ErrorKind::UnexpectedEnd, 3);
    }

    #[test]
    fn refuses_an_odd_number_of_hexadecimal_digits() {
        assert_refused(b"#616#", ErrorKind::OddHexDigits, 0);
        assert_refused(b"(1:a #f#)", ErrorKind::OddHexDigits, 5);
    }

    #[test]
    fn refuses_hexadecimal_where_the_parser_allows_none() {
        let parser = Parser::new().allow_hexadecimal(false);
        let error = parser.parse(b"#616263#").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::HexadecimalNotAllowed);
        assert_eq!(error.offset(), 0);
        assert_eq!(parser.parse(b"|YWJj|").unwrap(), Sexp::atom("abc"));
    }

    // Base 64 (§4.5)

    #[test]
    fn reads_a_base_64_octet_string() {
        assert_eq!(parse(b"|YWJj|").unwrap(), Sexp::atom("abc"));
        assert_eq!(parse(b"|Zg==|").unwrap(), Sexp::atom("f"));
        assert_eq!(parse(b"||").unwrap(), Sexp::atom(""));
    }

    #[test]
    fn reads_whitespace_inside_a_base_64_octet_string() {
        assert_eq!(parse(b"|YW Jj|").unwrap(), Sexp::atom("abc"));
        assert_eq!(parse(b"|\nZg==\n|").unwrap(), Sexp::atom("f"));
    }

    #[test]
    fn refuses_base_64_that_is_not_base_64() {
        assert_refused(b"|YW*j|", ErrorKind::InvalidBase64, 3);
        assert_refused(b"|YWJj", ErrorKind::UnexpectedEnd, 5);
    }

    #[test]
    fn refuses_base_64_that_ends_in_the_middle_of_a_group() {
        assert_refused(b"|Y|", ErrorKind::InvalidBase64, 1);
        assert_refused(b"|YWJja|", ErrorKind::InvalidBase64, 5);
    }

    #[test]
    fn refuses_base_64_whose_last_character_carries_bits_it_should_not() {
        assert_refused(b"|Zh==|", ErrorKind::InvalidBase64, 2);
    }

    #[test]
    fn refuses_base_64_where_the_parser_allows_none() {
        let parser = Parser::new().allow_base64(false);
        let error = parser.parse(b"(1:a|YWJj|)").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::Base64NotAllowed);
        assert_eq!(error.offset(), 4);
        assert_eq!(parser.parse(b"#616263#").unwrap(), Sexp::atom("abc"));
    }

    #[test]
    fn refusing_base_64_octet_strings_leaves_the_braced_form_alone() {
        let parser = Parser::new().allow_base64(false);

        assert_eq!(parser.parse(b"{KCk=}").unwrap(), Sexp::list([]));
    }

    // Forms together

    #[test]
    fn reads_forms_mixed_within_one_list() {
        let expected = crate::sexp!["a", "b c", Sexp::atom([0xff]), "d", ""];

        assert_eq!(parse(br#"(a "b c" #ff# 1:d ||)"#).unwrap(), expected);
    }

    #[test]
    fn reads_one_octet_string_from_every_form_that_can_hold_it() {
        let expected = Sexp::atom("abc");

        assert_eq!(parse(b"3:abc").unwrap(), expected);
        assert_eq!(parse(b"abc").unwrap(), expected);
        assert_eq!(parse(br#""abc""#).unwrap(), expected);
        assert_eq!(parse(b"#616263#").unwrap(), expected);
        assert_eq!(parse(b"|YWJj|").unwrap(), expected);
    }

    // Octet strings the parser was told to refuse

    #[test]
    fn refuses_an_advanced_form_where_only_the_canonical_one_is_read() {
        let parser = Parser::canonical();

        for input in [b"issuer".as_slice(), br#""abc""#, b"#616263#", b"|YWJj|"] {
            let error = parser.parse(input).unwrap_err();

            assert_eq!(
                error.kind(),
                ErrorKind::AdvancedNotAllowed,
                "parsing {input:?}"
            );
            assert_eq!(error.offset(), 0);
        }

        assert_eq!(parser.parse(b"3:abc").unwrap(), Sexp::atom("abc"));
    }

    #[test]
    fn refuses_an_advanced_form_that_a_length_begins_at_the_length() {
        let error = Parser::canonical().parse(b"3#616263#").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::AdvancedNotAllowed);
        assert_eq!(error.offset(), 0);
    }

    #[test]
    fn refuses_a_zero_length_octet_string_where_the_parser_allows_none() {
        let parser = Parser::new().allow_empty_atoms(false);

        for input in [b"0:".as_slice(), br#""""#, b"##", b"||"] {
            let error = parser.parse(input).unwrap_err();

            assert_eq!(
                error.kind(),
                ErrorKind::EmptyAtomNotAllowed,
                "parsing {input:?}"
            );
        }

        assert_eq!(parser.parse(b"1:a").unwrap(), Sexp::atom("a"));
    }

    #[test]
    fn refuses_a_zero_length_display_hint_where_the_parser_allows_none() {
        let parser = Parser::new().allow_empty_atoms(false);
        let error = parser.parse(b"[0:]1:a").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::EmptyAtomNotAllowed);
        assert_eq!(error.offset(), 1);
    }

    #[test]
    fn refuses_an_octet_string_longer_than_the_parser_allows() {
        let parser = Parser::new().max_atom_len(3);
        let error = parser.parse(b"4:abcd").unwrap_err();

        assert_eq!(parser.parse(b"3:abc").unwrap(), Sexp::atom("abc"));
        assert_eq!(error.kind(), ErrorKind::AtomTooLong);
        assert_eq!(error.offset(), 0);
    }

    #[test]
    fn bounds_the_length_of_every_form_an_octet_string_takes() {
        let parser = Parser::new().max_atom_len(2);

        for input in [
            b"3:abc".as_slice(),
            br#""abc""#,
            b"abc",
            b"#616263#",
            b"|YWJj|",
        ] {
            let error = parser.parse(input).unwrap_err();

            assert_eq!(error.kind(), ErrorKind::AtomTooLong, "parsing {input:?}");
        }
    }

    #[test]
    fn bounds_the_length_of_a_display_hint_of_its_own() {
        let parser = Parser::new().max_atom_len(3);
        let error = parser.parse(b"[4:hint]1:a").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::AtomTooLong);
        assert_eq!(error.offset(), 1);
    }
}
