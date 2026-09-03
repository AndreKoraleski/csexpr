//! One pass over one input.
//!
//! A [`Reader`] holds the octets, how far into them it has gone, and the
//! [`Parser`] that says what it will accept. This module reads what §7.1 calls
//! a value, which is a list or an octet string with the display hint that may
//! precede it. The forms an octet string itself takes are read in
//! [`octet_string`].
//!
//! Nothing here recurses. A list still open is held on a stack with the offset
//! it opened at, and is built once its last element is in hand, so input that
//! nests deeply costs heap rather than stack.

mod octet_string;
mod quoted;

use crate::base64;
use crate::decode::error::{Error, ErrorKind};
use crate::decode::parser::Parser;
use crate::syntax;
use crate::types::{Atom, Sexp};

/// A reader part way through one input.
pub(super) struct Reader<'a> {
    parser: &'a Parser,
    input: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    /// Creates a reader over the whole of `input`.
    pub(super) fn new(parser: &'a Parser, input: &'a [u8]) -> Self {
        Self {
            parser,
            input,
            offset: 0,
        }
    }

    /// Returns how far into the input the reader has gone, in octets.
    pub(super) fn offset(&self) -> usize {
        self.offset
    }

    /// Reads one S-expression, in whichever representation it is written.
    pub(super) fn sexp(&mut self) -> Result<Sexp, Error> {
        self.skip_whitespace();

        if self.peek() == Some(b'{') {
            return self.transport();
        }

        self.value()
    }

    /// Passes over whitespace, where the representation being read has any.
    ///
    /// The canonical representation has none, so a parser restricted to it
    /// stops here and lets the whitespace be refused where it stands.
    pub(super) fn skip_whitespace(&mut self) {
        if !self.parser.advanced {
            return;
        }

        while let Some(octet) = self.peek() {
            if !syntax::is_whitespace(octet) {
                break;
            }

            self.offset += 1;
        }
    }

    /// Reads one S-expression, without recursing into its lists.
    fn value(&mut self) -> Result<Sexp, Error> {
        let mut open: Vec<(usize, Vec<Sexp>)> = Vec::new();

        loop {
            self.skip_whitespace();

            let value = match self.peek_required()? {
                b'(' => {
                    let start = self.offset;

                    if open.len() >= self.parser.max_depth {
                        return Err(Error::new(ErrorKind::TooDeep, start));
                    }

                    self.offset += 1;
                    open.push((start, Vec::new()));

                    continue;
                }
                b')' => {
                    let (start, items) = open
                        .pop()
                        .ok_or_else(|| self.error(ErrorKind::UnmatchedParenthesis))?;

                    self.offset += 1;
                    self.check_list(&items, start)?;

                    Sexp::List(items)
                }
                _ => Sexp::Atom(self.string()?),
            };

            match open.last_mut() {
                Some((_, parent)) => parent.push(value),
                None => return Ok(value),
            }
        }
    }

    /// Reads a whole S-expression encoded in base 64 between braces (§6.1).
    ///
    /// What the braces hold is a canonical representation, so it is read by a
    /// parser restricted to that, carrying every other restriction from the
    /// parser that reached the braces.
    fn transport(&mut self) -> Result<Sexp, Error> {
        let start = self.offset;

        if !self.parser.transport {
            return Err(self.error(ErrorKind::TransportNotAllowed));
        }

        self.offset += 1;

        let end = self.find(b'}')?;
        let octets = base64::decode(&self.input[self.offset..end]).map_err(|offset| {
            Error::new(ErrorKind::InvalidBase64, self.offset.saturating_add(offset))
        })?;

        self.offset = end + 1;

        // The octets inside the braces are the result of decoding and appear
        // at no offset of the input, so a failure among them is reported where
        // the braces are.
        self.parser
            .within_transport()
            .parse(&octets)
            .map_err(|error| Error::new(error.kind(), start))
    }

    /// Reads one octet string, with the display hint before it if there is
    /// one.
    fn string(&mut self) -> Result<Atom, Error> {
        let hint = match self.peek() {
            Some(b'[') => Some(self.display_hint()?),
            _ => None,
        };

        match hint {
            Some(hint) => {
                if self.peek() == Some(b'(') {
                    return Err(self.error(ErrorKind::HintOnList));
                }

                Ok(Atom::from(self.simple_string()?).with_hint(hint))
            }
            None => Ok(Atom::from(self.simple_string()?)),
        }
    }

    /// Reads a display hint, brackets and all (§4.6).
    fn display_hint(&mut self) -> Result<Vec<u8>, Error> {
        if !self.parser.hints {
            return Err(self.error(ErrorKind::HintNotAllowed));
        }

        self.offset += 1;
        self.skip_whitespace();

        let hint = self.simple_string()?;

        self.skip_whitespace();

        if self.peek_required()? != b']' {
            return Err(self.error(ErrorKind::UnexpectedOctet));
        }

        self.offset += 1;
        self.skip_whitespace();

        Ok(hint)
    }

    /// Returns the offset of the next `octet`, or reports that the input ended
    /// before one appeared.
    fn find(&self, octet: u8) -> Result<usize, Error> {
        self.input[self.offset..]
            .iter()
            .position(|candidate| *candidate == octet)
            .map(|index| self.offset + index)
            .ok_or_else(|| Error::new(ErrorKind::UnexpectedEnd, self.input.len()))
    }

    /// Returns the octet at the offset, if the input reaches that far.
    fn peek(&self) -> Option<u8> {
        self.input.get(self.offset).copied()
    }

    /// Returns the octet at the offset, or reports that the input ended.
    fn peek_required(&self) -> Result<u8, Error> {
        self.peek()
            .ok_or_else(|| Error::new(ErrorKind::UnexpectedEnd, self.input.len()))
    }

    /// Creates an error at the offset the reader stands on.
    fn error(&self, kind: ErrorKind) -> Error {
        Error::new(kind, self.offset)
    }

    /// Reports a length that disagrees with the octets it counts.
    ///
    /// §4.2, §4.4 and §4.5 allow a length before the form they describe, which
    /// states in advance how many octets it decodes to.
    fn check_declared(
        &self,
        declared: Option<usize>,
        actual: usize,
        start: usize,
    ) -> Result<(), Error> {
        match declared {
            Some(len) if len != actual => Err(Error::new(ErrorKind::LengthMismatch, start)),
            _ => Ok(()),
        }
    }

    /// Reports a list the parser was told to refuse.
    fn check_list(&self, items: &[Sexp], start: usize) -> Result<(), Error> {
        if items.is_empty() && !self.parser.empty_lists {
            return Err(Error::new(ErrorKind::EmptyListNotAllowed, start));
        }

        if !self.parser.list_as_first_element && items.first().is_some_and(Sexp::is_list) {
            return Err(Error::new(ErrorKind::ListAsFirstElementNotAllowed, start));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{DEFAULT_MAX_DEPTH, parse};
    use crate::sexp;

    /// A nesting deeper than any stack the tests may assume.
    const DEEP: usize = 100_000;

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

    /// Asserts that the input is refused for the given reason, at the given
    /// offset.
    fn assert_refused(input: &[u8], kind: ErrorKind, offset: usize) {
        let error = parse(input).unwrap_err();
        let seen = String::from_utf8_lossy(input).into_owned();

        assert_eq!(error.kind(), kind, "parsing {seen}");
        assert_eq!(error.offset(), offset, "parsing {seen}");
    }

    // Lists

    #[test]
    fn reads_a_list() {
        assert_eq!(parse(b"(6:issuer3:bob)").unwrap(), sexp!["issuer", "bob"]);
        assert_eq!(parse(b"(issuer bob)").unwrap(), sexp!["issuer", "bob"]);
    }

    #[test]
    fn reads_the_empty_list() {
        assert_eq!(parse(b"()").unwrap(), Sexp::list([]));
        assert_eq!(parse(b"(   )").unwrap(), Sexp::list([]));
    }

    #[test]
    fn reads_nested_lists() {
        let expected = sexp![sexp![], sexp!["a", sexp!["b"]]];

        assert_eq!(parse(b"(()(1:a(1:b)))").unwrap(), expected);
        assert_eq!(parse(b"(() (a (b)))").unwrap(), expected);
    }

    #[test]
    fn reads_elements_in_the_order_they_stand() {
        let sexp = parse(b"(1:a1:b1:c)").unwrap();

        assert_eq!(sexp.get(0), Some(&Sexp::atom("a")));
        assert_eq!(sexp.get(1), Some(&Sexp::atom("b")));
        assert_eq!(sexp.get(2), Some(&Sexp::atom("c")));
    }

    #[test]
    fn refuses_a_list_that_does_not_close() {
        assert_refused(b"(a b", ErrorKind::UnexpectedEnd, 4);
        assert_refused(b"(", ErrorKind::UnexpectedEnd, 1);
    }

    #[test]
    fn refuses_a_parenthesis_that_closes_nothing() {
        assert_refused(b")", ErrorKind::UnmatchedParenthesis, 0);
        assert_refused(b"(a))", ErrorKind::TrailingOctets, 3);
    }

    #[test]
    fn refuses_nothing_at_all() {
        assert_refused(b"", ErrorKind::UnexpectedEnd, 0);
        assert_refused(b"   ", ErrorKind::UnexpectedEnd, 3);
    }

    #[test]
    fn refuses_an_octet_that_can_begin_nothing() {
        assert_refused(b"\x00", ErrorKind::UnexpectedOctet, 0);
        assert_refused(b"(a ])", ErrorKind::UnexpectedOctet, 3);
    }

    // Whitespace

    #[test]
    fn reads_whitespace_around_an_s_expression() {
        assert_eq!(parse(b"  (a b)  ").unwrap(), sexp!["a", "b"]);
        assert_eq!(parse(b"\n4:data\n").unwrap(), Sexp::atom("data"));
    }

    #[test]
    fn reads_every_octet_section_three_counts_as_whitespace() {
        let expected = sexp!["a", "b"];

        assert_eq!(parse(b"(a b)").unwrap(), expected);
        assert_eq!(parse(b"(a\tb)").unwrap(), expected);
        assert_eq!(parse(b"(a\x0bb)").unwrap(), expected);
        assert_eq!(parse(b"(a\x0cb)").unwrap(), expected);
        assert_eq!(parse(b"(a\rb)").unwrap(), expected);
        assert_eq!(parse(b"(a\nb)").unwrap(), expected);
    }

    #[test]
    fn refuses_whitespace_where_only_the_canonical_representation_is_read() {
        let parser = Parser::canonical();

        assert!(parser.parse(b" 4:data").is_err());
        assert!(parser.parse(b"(1:a 1:b)").is_err());
        assert_eq!(parser.parse(b"(1:a1:b)").unwrap(), sexp!["a", "b"]);
    }

    // Display hints

    #[test]
    fn reads_a_display_hint() {
        let expected = Sexp::from(Atom::new("data").with_hint("hint"));

        assert_eq!(parse(b"[4:hint]4:data").unwrap(), expected);
        assert_eq!(parse(b"[hint]data").unwrap(), expected);
    }

    #[test]
    fn reads_a_zero_length_display_hint() {
        let expected = Sexp::from(Atom::new("data").with_hint(""));

        assert_eq!(parse(b"[0:]4:data").unwrap(), expected);
    }

    #[test]
    fn reads_whitespace_within_and_after_a_display_hint() {
        let expected = Sexp::from(Atom::new("bob").with_hint("hint"));

        assert_eq!(parse(b"[ hint ] bob").unwrap(), expected);
        assert_eq!(parse(b"[\nhint\n]\nbob").unwrap(), expected);
    }

    #[test]
    fn reads_a_display_hint_on_an_element_of_a_list() {
        let expected = sexp![Atom::new("a").with_hint("h"), "b"];

        assert_eq!(parse(b"([1:h]1:a1:b)").unwrap(), expected);
    }

    #[test]
    fn refuses_a_display_hint_before_a_list() {
        assert_refused(b"[4:hint](1:a)", ErrorKind::HintOnList, 8);
        assert_refused(b"[hint] (a)", ErrorKind::HintOnList, 7);
    }

    #[test]
    fn refuses_a_display_hint_that_does_not_close() {
        assert_refused(b"[4:hint4:data", ErrorKind::UnexpectedOctet, 7);
        assert_refused(b"[4:hint", ErrorKind::UnexpectedEnd, 7);
    }

    #[test]
    fn refuses_a_display_hint_on_a_display_hint() {
        assert_refused(b"[[1:a]1:b]1:c", ErrorKind::UnexpectedOctet, 1);
    }

    #[test]
    fn refuses_a_display_hint_where_the_parser_allows_none() {
        let parser = Parser::new().allow_display_hints(false);
        let error = parser.parse(b"[4:hint]4:data").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::HintNotAllowed);
        assert_eq!(error.offset(), 0);
        assert_eq!(parser.parse(b"4:data").unwrap(), Sexp::atom("data"));
    }

    // The basic transport representation

    #[test]
    fn reads_a_basic_transport_representation() {
        assert_eq!(
            parse(b"{KDY6aXNzdWVyMzpib2Ip}").unwrap(),
            sexp!["issuer", "bob"]
        );
        assert_eq!(parse(b"{KCk=}").unwrap(), Sexp::list([]));
        assert_eq!(parse(b"{NDpkYXRh}").unwrap(), Sexp::atom("data"));
    }

    #[test]
    fn reads_whitespace_inside_a_basic_transport_representation() {
        let expected = sexp!["a", sexp!["b"]];

        assert_eq!(parse(b"{KDE6 YSgx\nOmIpKQ==}").unwrap(), expected);
    }

    #[test]
    fn refuses_an_advanced_representation_inside_the_braces() {
        // The braces hold a canonical representation and nothing else.
        assert_refused(b"{KGlzc3VlciBib2Ip}", ErrorKind::AdvancedNotAllowed, 0);
    }

    #[test]
    fn refuses_braces_anywhere_but_around_the_whole_input() {
        assert_refused(b"({KCk=})", ErrorKind::UnexpectedOctet, 1);
    }

    #[test]
    fn refuses_braces_that_do_not_close() {
        assert_refused(b"{KCk=", ErrorKind::UnexpectedEnd, 5);
    }

    #[test]
    fn reports_a_failure_inside_the_braces_at_the_braces() {
        // The octets the failure is in are decoded, so they stand at no offset
        // of the input.
        assert_refused(b"  {KDY6}", ErrorKind::UnexpectedEnd, 2);
    }

    #[test]
    fn reports_base_64_that_is_not_base_64_where_it_stands() {
        assert_refused(b"{KDY6*}", ErrorKind::InvalidBase64, 5);
    }

    #[test]
    fn refuses_the_braced_form_where_the_parser_allows_none() {
        let parser = Parser::new().allow_transport(false);
        let error = parser.parse(b"{KCk=}").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::TransportNotAllowed);
        assert_eq!(error.offset(), 0);
        assert_eq!(parser.parse(b"()").unwrap(), Sexp::list([]));
    }

    #[test]
    fn carries_every_other_restriction_inside_the_braces() {
        let parser = Parser::new().allow_empty_lists(false);
        let error = parser.parse(b"{KCk=}").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::EmptyListNotAllowed);
    }

    // Lists the parser was told to refuse

    #[test]
    fn refuses_an_empty_list_where_the_parser_allows_none() {
        let parser = Parser::new().allow_empty_lists(false);
        let error = parser.parse(b"(1:a())").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::EmptyListNotAllowed);
        assert_eq!(error.offset(), 4);
        assert_eq!(parser.parse(b"(1:a)").unwrap(), sexp!["a"]);
    }

    #[test]
    fn refuses_a_list_that_begins_with_a_list_where_the_parser_allows_none() {
        let parser = Parser::new().allow_list_as_first_element(false);
        let error = parser.parse(b"((1:a)1:b)").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::ListAsFirstElementNotAllowed);
        assert_eq!(error.offset(), 0);
        assert_eq!(parser.parse(b"(1:b(1:a))").unwrap(), sexp!["b", sexp!["a"]]);
    }

    // Depth

    #[test]
    fn reads_to_the_depth_the_default_allows() {
        let within = [
            b"(".repeat(DEFAULT_MAX_DEPTH),
            b")".repeat(DEFAULT_MAX_DEPTH),
        ]
        .concat();
        let beyond = [
            b"(".repeat(DEFAULT_MAX_DEPTH + 1),
            b")".repeat(DEFAULT_MAX_DEPTH + 1),
        ]
        .concat();

        assert_eq!(parse(&within).unwrap().depth(), DEFAULT_MAX_DEPTH);
        assert_eq!(parse(&beyond).unwrap_err().kind(), ErrorKind::TooDeep);
    }

    #[test]
    fn refuses_nesting_beyond_what_the_parser_allows() {
        let parser = Parser::new().max_depth(2);
        let error = parser.parse(b"(((1:a)))").unwrap_err();

        assert_eq!(parser.parse(b"((1:a))").unwrap().depth(), 2);
        assert_eq!(error.kind(), ErrorKind::TooDeep);
        assert_eq!(error.offset(), 2);
    }

    #[test]
    fn refuses_every_list_where_the_parser_allows_no_nesting() {
        let parser = Parser::new().max_depth(0);

        assert_eq!(parser.parse(b"4:data").unwrap(), Sexp::atom("data"));
        assert_eq!(parser.parse(b"()").unwrap_err().kind(), ErrorKind::TooDeep);
    }

    #[test]
    fn reading_does_not_recurse() {
        let input = [b"(".repeat(DEEP), b")".repeat(DEEP)].concat();
        let sexp = Parser::new().max_depth(DEEP).parse(&input).unwrap();

        assert_eq!(sexp.depth(), DEEP);

        dismantle(sexp);
    }

    #[test]
    fn refusing_input_that_nests_too_deeply_does_not_recurse() {
        let error = parse(&b"(".repeat(DEEP)).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::TooDeep);
        assert_eq!(error.offset(), DEFAULT_MAX_DEPTH);
    }
}
