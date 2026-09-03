//! S-expressions, as specified by [RFC 9804].
//!
//! An S-expression is either an octet string or a list of simpler
//! S-expressions (§2). [`Atom`] is the octet string, carrying the optional
//! display hint of §4.6, and [`Sexp`] is the S-expression itself. Both are
//! values, and neither holds any trace of how it was written, since §6 gives
//! one S-expression three ways of being written and the choice among them
//! belongs to whoever writes it.
//!
//! [`encode`] writes an S-expression in the canonical representation of §6.2,
//! which is the one to hash or sign, in the basic transport representation of
//! §6.3, which survives a channel that would disturb raw octets, or in the
//! advanced representation of §6.4, which is meant to be read by a person.
//! [`decode`] reads all three, and narrows to whatever subset an application
//! accepts through the restrictions §8 lists.
//!
//! # Examples
//!
//! ```
//! use sexp::{Atom, decode, encode::canonical, sexp};
//!
//! let cert = sexp!["issuer", Atom::new("bob").with_hint("text/plain")];
//! let written = canonical::to_vec(&cert);
//!
//! assert_eq!(written, b"(6:issuer[10:text/plain]3:bob)");
//! assert_eq!(decode::parse(&written).unwrap(), cert);
//!
//! // The advanced representation of §6.4 carries the same value.
//! assert_eq!(cert.to_string(), "(issuer [text/plain]bob)");
//! assert_eq!(decode::parse(b"(issuer [text/plain]bob)").unwrap(), cert);
//! ```
//!
//! # Reading what cannot be trusted
//!
//! Parsing takes no stack in proportion to how deeply the input nests, and no
//! memory on the strength of a length the input states. What a parse builds is
//! a tree, and dropping, cloning or comparing a tree does recurse, so a parser
//! refuses input nested deeper than [`DEFAULT_MAX_DEPTH`] unless it is told
//! otherwise. [`decode::Parser`] bounds the size of the input and of every
//! octet string in it as well, and turns off whichever constructs an
//! application has no use for.
//!
//! This crate has no dependencies and contains no `unsafe` code.
//!
//! [RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html
//! [`DEFAULT_MAX_DEPTH`]: decode::DEFAULT_MAX_DEPTH
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod base64;
mod syntax;

pub mod decode;
pub mod encode;
pub mod types;

pub use crate::types::{Atom, Sexp};
