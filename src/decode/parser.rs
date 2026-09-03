//! What an application will accept, and reading input against it.
//!
//! Out of the box a [`Parser`] accepts every representation of RFC 9804 §6,
//! since a reader that meets the specification accepts them all. The
//! restrictions §8 lists are the ones that can be turned on, one call each.
//! The reading itself is [`crate::decode::reader`].

use std::io;
use std::io::Read as _;

use crate::decode::error::{Error, ErrorKind, ReadError};
use crate::decode::reader::Reader;
use crate::types::Sexp;

/// How deeply lists may nest before a parser that was given no other limit
/// refuses to go further.
///
/// RFC 9804 sets no such limit. This one is here because the value a parse
/// builds is a tree, and dropping, cloning or comparing a tree walks it by
/// recursion, so a tree deeper than the stack can hold is a hazard to whatever
/// receives it. A thousand and twenty-four is far past what an S-expression
/// written by hand or by a protocol reaches, and far short of what any stack
/// this crate runs on will bear.
pub const DEFAULT_MAX_DEPTH: usize = 1024;

/// A reader of S-expressions, and the restrictions it reads against.
///
/// [`new`] accepts the canonical representation of §6.2, the basic transport
/// representation of §6.3, and the advanced representation of §6.4, which
/// together are what §6 asks a reader to accept. [`canonical`] accepts only
/// the canonical representation, which is what verifying a signature calls
/// for, since a signature is computed over those octets exactly.
///
/// The rest of the methods impose the restrictions §8 lists. Each one is off
/// to begin with, so that a parser fresh from [`new`] refuses nothing the
/// specification admits, apart from nesting deeper than [`DEFAULT_MAX_DEPTH`].
/// They exist because §8 says an application may narrow what it accepts, and
/// narrowing is worth doing where the input comes from somewhere untrusted.
///
/// A parser holds no state between parses, so one may be built once and used
/// as often as wanted, from as many threads as wanted.
///
/// # Examples
///
/// ```
/// use sexp::{decode::Parser, sexp};
///
/// let parser = Parser::new();
/// let expected = sexp!["issuer", "bob"];
///
/// assert_eq!(parser.parse(b"(6:issuer3:bob)").unwrap(), expected);
/// assert_eq!(parser.parse(b"(issuer bob)").unwrap(), expected);
/// assert_eq!(parser.parse(b"{KDY6aXNzdWVyMzpib2Ip}").unwrap(), expected);
/// ```
///
/// [`new`]: Self::new
/// [`canonical`]: Self::canonical
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Parser {
    pub(super) max_depth: usize,
    pub(super) max_atom_len: usize,
    pub(super) max_input_len: usize,
    pub(super) advanced: bool,
    pub(super) transport: bool,
    pub(super) hints: bool,
    pub(super) empty_atoms: bool,
    pub(super) empty_lists: bool,
    pub(super) list_as_first_element: bool,
    pub(super) hexadecimal: bool,
    pub(super) base64: bool,
    pub(super) lengths: bool,
}

impl Parser {
    /// Creates a parser that accepts every representation of §6.
    ///
    /// Nothing the specification admits is refused, except lists nested deeper
    /// than [`DEFAULT_MAX_DEPTH`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            max_atom_len: usize::MAX,
            max_input_len: usize::MAX,
            advanced: true,
            transport: true,
            hints: true,
            empty_atoms: true,
            empty_lists: true,
            list_as_first_element: true,
            hexadecimal: true,
            base64: true,
            lengths: true,
        }
    }

    /// Creates a parser that accepts only the canonical representation of
    /// §6.2.
    ///
    /// Verifying a signature means knowing that the octets signed are the
    /// octets in hand, and only the canonical representation makes that so. A
    /// parser from [`new`] would accept a basic transport or advanced
    /// representation of the same S-expression, which carries the same value
    /// and different octets.
    ///
    /// This is [`new`] with [`allow_advanced`] and [`allow_transport`] turned
    /// off. The other restrictions are as [`new`] leaves them.
    ///
    /// [`new`]: Self::new
    /// [`allow_advanced`]: Self::allow_advanced
    /// [`allow_transport`]: Self::allow_transport
    #[must_use]
    pub fn canonical() -> Self {
        Self::new().allow_advanced(false).allow_transport(false)
    }

    /// Sets how deeply lists may nest.
    ///
    /// The outermost list counts as one, which is the depth [`Sexp::depth`]
    /// reports. Raising this far above [`DEFAULT_MAX_DEPTH`] gives back the
    /// hazard that constant is there to avoid, which is a tree too deep to
    /// drop, clone or compare without exhausting the stack.
    #[must_use]
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Sets how long one octet string may be, in octets.
    ///
    /// §8 names a fixed limit on the size of an octet string as a restriction
    /// an application may impose. The limit applies to the data of an atom and
    /// to its display hint separately, since §4.6 makes the hint an octet
    /// string of its own.
    #[must_use]
    pub fn max_atom_len(mut self, len: usize) -> Self {
        self.max_atom_len = len;
        self
    }

    /// Sets how long the whole input may be, in octets.
    ///
    /// This is measured before parsing begins, so input beyond the limit is
    /// refused without being examined. [`parse_reader`] stops reading there as
    /// well, rather than reading everything offered and refusing it after.
    ///
    /// [`parse_reader`]: Self::parse_reader
    #[must_use]
    pub fn max_input_len(mut self, len: usize) -> Self {
        self.max_input_len = len;
        self
    }

    /// Sets whether the advanced representation of §6.4 is accepted.
    ///
    /// Turning it off leaves the canonical representation, which is verbatim
    /// octet strings and lists and nothing else. Whitespace, tokens, quoted
    /// strings, hexadecimal and base 64 are then all refused, whatever the
    /// other restrictions say, since none of them appear in a canonical
    /// representation.
    #[must_use]
    pub fn allow_advanced(mut self, allow: bool) -> Self {
        self.advanced = allow;
        self
    }

    /// Sets whether the basic transport representation of §6.3 is accepted.
    ///
    /// This is the braced form of §6.1, which holds a canonical representation
    /// encoded in base 64. §7.3 puts it around a whole S-expression, so it is
    /// accepted only as the whole input, never as an element within a list.
    #[must_use]
    pub fn allow_transport(mut self, allow: bool) -> Self {
        self.transport = allow;
        self
    }

    /// Sets whether display hints are accepted (§8).
    ///
    /// §4.6 makes a display hint optional wherever an octet string appears, so
    /// an application that has no use for one may refuse it outright.
    #[must_use]
    pub fn allow_display_hints(mut self, allow: bool) -> Self {
        self.hints = allow;
        self
    }

    /// Sets whether zero-length octet strings are accepted (§8).
    ///
    /// The restriction applies to the data of an atom and to its display hint
    /// alike.
    #[must_use]
    pub fn allow_empty_atoms(mut self, allow: bool) -> Self {
        self.empty_atoms = allow;
        self
    }

    /// Sets whether empty lists are accepted (§8).
    #[must_use]
    pub fn allow_empty_lists(mut self, allow: bool) -> Self {
        self.empty_lists = allow;
        self
    }

    /// Sets whether a list may have a list as its first element (§8).
    ///
    /// An application that reads the first element of every list as a name for
    /// what the list holds has no use for a list that begins with a list.
    #[must_use]
    pub fn allow_list_as_first_element(mut self, allow: bool) -> Self {
        self.list_as_first_element = allow;
        self
    }

    /// Sets whether hexadecimal octet strings are accepted (§8).
    ///
    /// This is the form of §4.4, between two `#` characters. It appears only
    /// in the advanced representation.
    #[must_use]
    pub fn allow_hexadecimal(mut self, allow: bool) -> Self {
        self.hexadecimal = allow;
        self
    }

    /// Sets whether base-64 octet strings are accepted (§8).
    ///
    /// This is the form of §4.5, between two `|` characters, which is one
    /// octet string within an advanced representation. It has nothing to do
    /// with [`allow_transport`], which governs a whole S-expression encoded in
    /// base 64 between braces.
    ///
    /// [`allow_transport`]: Self::allow_transport
    #[must_use]
    pub fn allow_base64(mut self, allow: bool) -> Self {
        self.base64 = allow;
        self
    }

    /// Sets whether a length may precede a quoted, hexadecimal or base-64
    /// octet string (§8).
    ///
    /// §4.2, §4.4 and §4.5 allow a length there, which states in advance how
    /// many octets the form decodes to. The length before a verbatim octet
    /// string is not affected, since §4.1 has no form without it.
    #[must_use]
    pub fn allow_lengths(mut self, allow: bool) -> Self {
        self.lengths = allow;
        self
    }

    /// Reads one S-expression, which is the whole of `input`.
    ///
    /// Whitespace before and after is accepted where the advanced
    /// representation is, as §7.1 allows. Anything else left over is an error,
    /// which is what distinguishes this from [`parse_prefix`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] saying what was wrong and where.
    ///
    /// [`parse_prefix`]: Self::parse_prefix
    pub fn parse(&self, input: &[u8]) -> Result<Sexp, Error> {
        let mut reader = self.reader(input)?;
        let sexp = reader.sexp()?;

        reader.skip_whitespace();

        if reader.offset() < input.len() {
            return Err(Error::new(ErrorKind::TrailingOctets, reader.offset()));
        }

        Ok(sexp)
    }

    /// Reads one S-expression from the start of `input`, and returns it with
    /// the number of octets it occupied.
    ///
    /// What follows is not examined, so this is the way to read a stream of
    /// S-expressions one after another. The count does not include whitespace
    /// after the S-expression, only whatever was needed to read it.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] saying what was wrong and where.
    pub fn parse_prefix(&self, input: &[u8]) -> Result<(Sexp, usize), Error> {
        let mut reader = self.reader(input)?;
        let sexp = reader.sexp()?;

        Ok((sexp, reader.offset()))
    }

    /// Reads everything `input` yields, and parses it as [`parse`] does.
    ///
    /// No more than [`max_input_len`] octets are read, and one more than that
    /// only to tell input of exactly that length from input beyond it.
    ///
    /// # Errors
    ///
    /// Returns [`ReadError::Io`] if the reader failed, and
    /// [`ReadError::Parse`] if what it yielded is not an S-expression this
    /// parser accepts.
    ///
    /// [`parse`]: Self::parse
    /// [`max_input_len`]: Self::max_input_len
    pub fn parse_reader(&self, input: impl io::Read) -> Result<Sexp, ReadError> {
        let limit = u64::try_from(self.max_input_len).unwrap_or(u64::MAX);
        let mut octets = Vec::new();

        input
            .take(limit.saturating_add(1))
            .read_to_end(&mut octets)?;

        Ok(self.parse(&octets)?)
    }

    /// Returns this parser as it applies to what a pair of braces holds, which
    /// §6.1 makes a canonical representation.
    pub(super) fn within_transport(&self) -> Self {
        Self {
            advanced: false,
            transport: false,
            ..*self
        }
    }

    /// Creates a reader over input this parser will look at at all.
    fn reader<'a>(&'a self, input: &'a [u8]) -> Result<Reader<'a>, Error> {
        if input.len() > self.max_input_len {
            return Err(Error::new(ErrorKind::InputTooLong, self.max_input_len));
        }

        Ok(Reader::new(self, input))
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::{advanced, canonical, transport};
    use crate::sexp;
    use crate::types::Atom;

    #[test]
    fn new_restricts_nothing_but_the_depth() {
        let parser = Parser::new();

        assert_eq!(parser.max_depth, DEFAULT_MAX_DEPTH);
        assert_eq!(parser.max_atom_len, usize::MAX);
        assert_eq!(parser.max_input_len, usize::MAX);
        assert!(parser.advanced);
        assert!(parser.transport);
        assert!(parser.hints);
        assert!(parser.empty_atoms);
        assert!(parser.empty_lists);
        assert!(parser.list_as_first_element);
        assert!(parser.hexadecimal);
        assert!(parser.base64);
        assert!(parser.lengths);
    }

    #[test]
    fn default_max_depth_is_a_thousand_and_twenty_four() {
        assert_eq!(DEFAULT_MAX_DEPTH, 1024);
    }

    #[test]
    fn canonical_is_new_with_two_restrictions() {
        let expected = Parser::new().allow_advanced(false).allow_transport(false);

        assert_eq!(Parser::canonical(), expected);
    }

    #[test]
    fn default_is_new() {
        assert_eq!(Parser::default(), Parser::new());
    }

    #[test]
    fn every_restriction_is_kept_as_it_was_set() {
        let parser = Parser::new()
            .max_depth(3)
            .max_atom_len(4)
            .max_input_len(5)
            .allow_advanced(false)
            .allow_transport(false)
            .allow_display_hints(false)
            .allow_empty_atoms(false)
            .allow_empty_lists(false)
            .allow_list_as_first_element(false)
            .allow_hexadecimal(false)
            .allow_base64(false)
            .allow_lengths(false);

        assert_eq!(parser.max_depth, 3);
        assert_eq!(parser.max_atom_len, 4);
        assert_eq!(parser.max_input_len, 5);
        assert!(!parser.advanced);
        assert!(!parser.transport);
        assert!(!parser.hints);
        assert!(!parser.empty_atoms);
        assert!(!parser.empty_lists);
        assert!(!parser.list_as_first_element);
        assert!(!parser.hexadecimal);
        assert!(!parser.base64);
        assert!(!parser.lengths);
    }

    #[test]
    fn within_transport_keeps_every_other_restriction() {
        let parser = Parser::new().max_depth(3).allow_empty_lists(false);
        let inner = parser.within_transport();

        assert!(!inner.advanced);
        assert!(!inner.transport);
        assert_eq!(inner.max_depth, 3);
        assert!(!inner.empty_lists);
    }

    #[test]
    fn a_parser_may_be_used_more_than_once() {
        let parser = Parser::new();

        assert_eq!(parser.parse(b"1:a").unwrap(), Sexp::atom("a"));
        assert_eq!(parser.parse(b"1:b").unwrap(), Sexp::atom("b"));
    }

    // parse

    #[test]
    fn parse_reads_the_whole_input() {
        assert_eq!(Parser::new().parse(b"(1:a1:b)").unwrap(), sexp!["a", "b"]);
    }

    #[test]
    fn parse_refuses_octets_after_a_complete_s_expression() {
        let parser = Parser::new();
        let error = parser.parse(b"4:data4:more").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::TrailingOctets);
        assert_eq!(error.offset(), 6);
        assert_eq!(parser.parse(b"(a) (b)").unwrap_err().offset(), 4);
    }

    #[test]
    fn parse_accepts_whitespace_after_a_complete_s_expression() {
        assert_eq!(
            Parser::new().parse(b"4:data \n").unwrap(),
            Sexp::atom("data")
        );
    }

    #[test]
    fn parse_refuses_input_longer_than_allowed() {
        let parser = Parser::new().max_input_len(6);
        let error = parser.parse(b"4:datax").unwrap_err();

        assert_eq!(parser.parse(b"4:data").unwrap(), Sexp::atom("data"));
        assert_eq!(error.kind(), ErrorKind::InputTooLong);
        assert_eq!(error.offset(), 6);
    }

    // parse_prefix

    #[test]
    fn parse_prefix_reads_one_s_expression_and_says_how_far_it_read() {
        let parser = Parser::new();

        assert_eq!(
            parser.parse_prefix(b"4:data4:more").unwrap(),
            (Sexp::atom("data"), 6)
        );
        assert_eq!(parser.parse_prefix(b"(a) (b)").unwrap(), (sexp!["a"], 3));
    }

    #[test]
    fn parse_prefix_reads_a_stream_of_s_expressions() {
        let parser = Parser::new();
        let input = b"1:a1:b1:c";
        let mut offset = 0;
        let mut read = Vec::new();

        while offset < input.len() {
            let (sexp, len) = parser.parse_prefix(&input[offset..]).unwrap();

            read.push(sexp);
            offset += len;
        }

        assert_eq!(
            read,
            vec![Sexp::atom("a"), Sexp::atom("b"), Sexp::atom("c")]
        );
    }

    #[test]
    fn parse_prefix_counts_the_whitespace_it_had_to_pass_over() {
        let (sexp, len) = Parser::new().parse_prefix(b"  1:a1:b").unwrap();

        assert_eq!(sexp, Sexp::atom("a"));
        assert_eq!(len, 5);
    }

    // parse_reader

    #[test]
    fn parse_reader_reads_what_the_reader_yields() {
        let input: &[u8] = b"(6:issuer3:bob)";

        assert_eq!(
            Parser::new().parse_reader(input).unwrap(),
            sexp!["issuer", "bob"]
        );
    }

    #[test]
    fn parse_reader_reports_a_failure_of_the_reader() {
        struct Broken;

        impl io::Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
        }

        let failure = Parser::new().parse_reader(Broken).unwrap_err();

        assert!(matches!(failure, ReadError::Io(_)));
    }

    #[test]
    fn parse_reader_reports_a_failure_to_parse() {
        let failure = Parser::new().parse_reader(b"(a".as_slice()).unwrap_err();

        assert!(matches!(failure, ReadError::Parse(_)));
    }

    #[test]
    fn parse_reader_stops_at_the_input_limit() {
        let parser = Parser::new().max_input_len(3);
        let failure = parser.parse_reader(b"4:data".as_slice()).unwrap_err();

        match failure {
            ReadError::Parse(error) => assert_eq!(error.kind(), ErrorKind::InputTooLong),
            ReadError::Io(error) => panic!("reading failed with {error}"),
        }
    }

    #[test]
    fn parse_reader_accepts_input_of_exactly_the_limit() {
        let parser = Parser::new().max_input_len(6);

        assert_eq!(
            parser.parse_reader(b"4:data".as_slice()).unwrap(),
            Sexp::atom("data")
        );
    }

    // Reading back what was written

    /// Every shape a representation has to carry.
    fn corpus() -> Vec<Sexp> {
        vec![
            Sexp::atom(""),
            Sexp::atom("data"),
            Sexp::atom("a b"),
            Sexp::atom([0x00, 0xff, 0x1b, 0x7f]),
            Sexp::atom(vec![b'x'; 300]),
            Sexp::from(Atom::new("bob").with_hint("text/plain")),
            Sexp::from(Atom::new("").with_hint("")),
            Sexp::from(Atom::new([0xff]).with_hint([0x00])),
            Sexp::list([]),
            sexp!["issuer", "bob"],
            sexp![sexp![], sexp!["a", sexp!["b", "c"]]],
        ]
    }

    #[test]
    fn reads_back_what_the_canonical_writer_wrote() {
        let parser = Parser::new();

        for sexp in corpus() {
            assert_eq!(parser.parse(&canonical::to_vec(&sexp)).unwrap(), sexp);
        }
    }

    #[test]
    fn reads_back_what_the_advanced_writer_wrote() {
        let parser = Parser::new();

        for sexp in corpus() {
            let written = advanced::to_string(&sexp);

            assert_eq!(parser.parse(written.as_bytes()).unwrap(), sexp);
        }
    }

    #[test]
    fn reads_back_what_the_transport_writer_wrote() {
        let parser = Parser::new();

        for sexp in corpus() {
            assert_eq!(parser.parse(&transport::to_vec(&sexp)).unwrap(), sexp);
        }
    }

    #[test]
    fn reads_the_three_representations_of_one_value_as_one_value() {
        let parser = Parser::new();

        for sexp in corpus() {
            let canonical = parser.parse(&canonical::to_vec(&sexp)).unwrap();
            let advanced = parser.parse(advanced::to_string(&sexp).as_bytes()).unwrap();
            let transport = parser.parse(&transport::to_vec(&sexp)).unwrap();

            assert_eq!(canonical, advanced);
            assert_eq!(advanced, transport);
        }
    }

    #[test]
    fn writes_back_the_canonical_representation_it_read() {
        let parser = Parser::canonical();

        for sexp in corpus() {
            let written = canonical::to_vec(&sexp);
            let read = parser.parse(&written).unwrap();

            assert_eq!(canonical::to_vec(&read), written);
        }
    }
}
