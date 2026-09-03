//! Reading an S-expression from any representation of RFC 9804 §6.
//!
//! [`parse`] reads whatever representation the input is written in, which is
//! what §6 asks a reader to accept. [`parse_canonical`] reads only the
//! canonical representation of §6.2, which is what verifying a signature calls
//! for. [`Parser`] is either of those with the restrictions of §8 turned on
//! one at a time, and with limits on how deep and how large the input may be.
//!
//! A parse builds a [`Sexp`], which holds no trace of the representation it
//! came from. Reading and writing therefore preserve the value and not the
//! octets, and only the canonical representation is preserved octet for octet,
//! since §6.2 gives an S-expression exactly one of those.
//!
//! Parsing is iterative, so input that nests deeply costs no stack, and a
//! [`Parser`] refuses input nested deeper than [`DEFAULT_MAX_DEPTH`] unless it
//! is told otherwise.

mod error;
mod parser;
mod reader;

pub use error::{Error, ErrorKind, ReadError};
pub use parser::{DEFAULT_MAX_DEPTH, Parser};

use crate::types::Sexp;

/// Reads one S-expression, in whichever representation of §6 it is written.
///
/// This is [`Parser::new`] with nothing restricted beyond its default depth.
/// Build a [`Parser`] where the input comes from somewhere untrusted, or where
/// only one representation is to be accepted.
///
/// # Errors
///
/// Returns an [`Error`] saying what was wrong and where.
///
/// # Examples
///
/// ```
/// use sexp::{decode, sexp};
///
/// assert_eq!(
///     decode::parse(b"(6:issuer3:bob)").unwrap(),
///     sexp!["issuer", "bob"]
/// );
/// assert_eq!(
///     decode::parse(b"(issuer bob)").unwrap(),
///     sexp!["issuer", "bob"]
/// );
/// assert!(decode::parse(b"(issuer bob").is_err());
/// ```
pub fn parse(input: &[u8]) -> Result<Sexp, Error> {
    Parser::new().parse(input)
}

/// Reads one S-expression written in the canonical representation of §6.2.
///
/// This is [`Parser::canonical`], which refuses the advanced and basic
/// transport representations. Read this way where the octets themselves
/// matter, as they do to a signature computed over them.
///
/// # Errors
///
/// Returns an [`Error`] saying what was wrong and where.
///
/// # Examples
///
/// ```
/// use sexp::{decode, sexp};
///
/// assert_eq!(
///     decode::parse_canonical(b"(6:issuer3:bob)").unwrap(),
///     sexp!["issuer", "bob"]
/// );
/// assert!(decode::parse_canonical(b"(issuer bob)").is_err());
/// ```
pub fn parse_canonical(input: &[u8]) -> Result<Sexp, Error> {
    Parser::canonical().parse(input)
}
