//! What a parse reports when it stops.
//!
//! An [`Error`] says what was wrong and where, in the form of an [`ErrorKind`]
//! and an offset. The kinds fall into two groups. Some name a place where the
//! input departs from the grammar of RFC 9804 §7, and the rest name a
//! restriction of §8 that [`crate::decode::Parser`] was asked to impose and
//! the input broke.

use std::error;
use std::fmt;
use std::io;

/// What was wrong with the input, and where.
///
/// The offset is where the offending construct begins, counted in octets from
/// the start of the input the parser was given. An error found inside a basic
/// transport representation is reported at the opening brace, since the octets
/// it was found in are the result of decoding and appear nowhere in the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Error {
    kind: ErrorKind,
    offset: usize,
}

impl Error {
    /// Creates an error of the given kind, at the given offset.
    pub(crate) fn new(kind: ErrorKind, offset: usize) -> Self {
        Self { kind, offset }
    }

    /// Returns what was wrong.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns where in the input the offending construct begins, in octets.
    pub fn offset(&self) -> usize {
        self.offset
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at offset {}", self.kind, self.offset)
    }
}

impl error::Error for Error {}

/// What was wrong with the input.
///
/// More kinds may be added, so match with a wildcard arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The input ended in the middle of an S-expression.
    UnexpectedEnd,
    /// An octet appeared where the grammar admits no such octet.
    UnexpectedOctet,
    /// Octets follow an S-expression that is already complete.
    TrailingOctets,
    /// A closing parenthesis appeared with no list open.
    UnmatchedParenthesis,
    /// A length began with a zero and went on, which §7 forbids.
    LengthLeadingZero,
    /// A length was too large to represent.
    LengthOverflow,
    /// A length disagreed with the octet string it counts.
    LengthMismatch,
    /// A backslash in a quoted string began no escape §4.2 defines.
    InvalidEscape,
    /// An octet inside `#` and `#` was neither a hexadecimal digit nor
    /// whitespace.
    InvalidHexDigit,
    /// A hexadecimal octet string held an odd number of digits.
    OddHexDigits,
    /// An octet inside a base-64 encoding was not a base-64 character, or the
    /// encoding did not end as RFC 4648 requires.
    InvalidBase64,
    /// A display hint preceded a list, which §7 admits only before an octet
    /// string.
    HintOnList,
    /// Lists nested deeper than the parser allows.
    TooDeep,
    /// An octet string was longer than the parser allows.
    AtomTooLong,
    /// The input was longer than the parser allows.
    InputTooLong,
    /// A display hint appeared, and the parser allows none (§8).
    HintNotAllowed,
    /// A zero-length octet string appeared, and the parser allows none (§8).
    EmptyAtomNotAllowed,
    /// An empty list appeared, and the parser allows none (§8).
    EmptyListNotAllowed,
    /// A list began with a list, and the parser allows no such list (§8).
    ListAsFirstElementNotAllowed,
    /// A construct of the advanced representation appeared, and the parser
    /// allows only the canonical representation (§8).
    AdvancedNotAllowed,
    /// A basic transport representation appeared, and the parser allows none.
    TransportNotAllowed,
    /// A hexadecimal octet string appeared, and the parser allows none (§8).
    HexadecimalNotAllowed,
    /// A base-64 octet string appeared, and the parser allows none (§8).
    Base64NotAllowed,
    /// A length preceded a quoted, hexadecimal, or base-64 octet string, and
    /// the parser allows none there (§8).
    LengthNotAllowed,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let said = match self {
            Self::UnexpectedEnd => "the input ended in the middle of an S-expression",
            Self::UnexpectedOctet => "an octet that cannot appear here",
            Self::TrailingOctets => "octets after a complete S-expression",
            Self::UnmatchedParenthesis => "a closing parenthesis with no list open",
            Self::LengthLeadingZero => "a length with a leading zero",
            Self::LengthOverflow => "a length too large to represent",
            Self::LengthMismatch => "a length that disagrees with the octets it counts",
            Self::InvalidEscape => "an escape that is not defined",
            Self::InvalidHexDigit => "an octet that is not a hexadecimal digit",
            Self::OddHexDigits => "an odd number of hexadecimal digits",
            Self::InvalidBase64 => "an octet that does not belong in a base-64 encoding",
            Self::HintOnList => "a display hint before a list",
            Self::TooDeep => "lists nested deeper than allowed",
            Self::AtomTooLong => "an octet string longer than allowed",
            Self::InputTooLong => "input longer than allowed",
            Self::HintNotAllowed => "a display hint, which is not allowed",
            Self::EmptyAtomNotAllowed => "a zero-length octet string, which is not allowed",
            Self::EmptyListNotAllowed => "an empty list, which is not allowed",
            Self::ListAsFirstElementNotAllowed => {
                "a list as the first element of a list, which is not allowed"
            }
            Self::AdvancedNotAllowed => "the advanced representation, which is not allowed",
            Self::TransportNotAllowed => "the basic transport representation, which is not allowed",
            Self::HexadecimalNotAllowed => "a hexadecimal octet string, which is not allowed",
            Self::Base64NotAllowed => "a base-64 octet string, which is not allowed",
            Self::LengthNotAllowed => "a length before an octet string, which is not allowed",
        };

        f.write_str(said)
    }
}

/// What reading and parsing reports when either one stops.
///
/// Returned by [`crate::decode::Parser::parse_reader`], where the input has to
/// be read before it can be parsed and either step may fail.
#[derive(Debug)]
#[non_exhaustive]
pub enum ReadError {
    /// The reader failed before the input was in hand.
    Io(io::Error),
    /// The input was read, and it is not an S-expression the parser accepts.
    Parse(Error),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "the input could not be read, {error}"),
            Self::Parse(error) => error.fmt(f),
        }
    }
}

impl error::Error for ReadError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
        }
    }
}

impl From<io::Error> for ReadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<Error> for ReadError {
    fn from(error: Error) -> Self {
        Self::Parse(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    /// Every kind, so that a rule about all of them is tested on all of them.
    const KINDS: &[ErrorKind] = &[
        ErrorKind::UnexpectedEnd,
        ErrorKind::UnexpectedOctet,
        ErrorKind::TrailingOctets,
        ErrorKind::UnmatchedParenthesis,
        ErrorKind::LengthLeadingZero,
        ErrorKind::LengthOverflow,
        ErrorKind::LengthMismatch,
        ErrorKind::InvalidEscape,
        ErrorKind::InvalidHexDigit,
        ErrorKind::OddHexDigits,
        ErrorKind::InvalidBase64,
        ErrorKind::HintOnList,
        ErrorKind::TooDeep,
        ErrorKind::AtomTooLong,
        ErrorKind::InputTooLong,
        ErrorKind::HintNotAllowed,
        ErrorKind::EmptyAtomNotAllowed,
        ErrorKind::EmptyListNotAllowed,
        ErrorKind::ListAsFirstElementNotAllowed,
        ErrorKind::AdvancedNotAllowed,
        ErrorKind::TransportNotAllowed,
        ErrorKind::HexadecimalNotAllowed,
        ErrorKind::Base64NotAllowed,
        ErrorKind::LengthNotAllowed,
    ];

    // Error

    #[test]
    fn error_keeps_the_kind_and_the_offset() {
        let error = Error::new(ErrorKind::UnexpectedEnd, 7);

        assert_eq!(error.kind(), ErrorKind::UnexpectedEnd);
        assert_eq!(error.offset(), 7);
    }

    #[test]
    fn error_compares_by_kind_and_offset() {
        let error = Error::new(ErrorKind::UnexpectedEnd, 7);

        assert_eq!(error, Error::new(ErrorKind::UnexpectedEnd, 7));
        assert_ne!(error, Error::new(ErrorKind::UnexpectedEnd, 8));
        assert_ne!(error, Error::new(ErrorKind::UnexpectedOctet, 7));
    }

    #[test]
    fn error_displays_the_kind_and_the_offset() {
        let error = Error::new(ErrorKind::TrailingOctets, 12);

        assert_eq!(
            error.to_string(),
            "octets after a complete S-expression at offset 12"
        );
    }

    #[test]
    fn error_has_no_source() {
        assert!(Error::new(ErrorKind::UnexpectedEnd, 0).source().is_none());
    }

    // ErrorKind

    #[test]
    fn every_kind_displays_something() {
        for kind in KINDS {
            let said = kind.to_string();

            assert!(!said.is_empty());
            assert!(!said.ends_with('.'));
        }
    }

    #[test]
    fn no_two_kinds_display_the_same() {
        for (index, kind) in KINDS.iter().enumerate() {
            for other in &KINDS[index + 1..] {
                assert_ne!(kind.to_string(), other.to_string());
            }
        }
    }

    // ReadError

    #[test]
    fn read_error_carries_a_failure_to_read() {
        let failure = io::Error::from(io::ErrorKind::UnexpectedEof);
        let error = ReadError::from(failure);

        assert!(matches!(error, ReadError::Io(_)));
        assert!(error.source().is_some());
        assert!(error.to_string().starts_with("the input could not be read"));
    }

    #[test]
    fn read_error_carries_a_failure_to_parse() {
        let failure = Error::new(ErrorKind::UnexpectedEnd, 3);
        let error = ReadError::from(failure);

        assert!(matches!(error, ReadError::Parse(_)));
        assert!(error.source().is_some());
        assert_eq!(error.to_string(), failure.to_string());
    }
}
