//! S-expressions, as specified by [RFC 9804].
//!
//! An S-expression is either an octet string or a list of simpler
//! S-expressions (§2). [`Atom`] is the octet string, carrying the optional
//! display hint of §4.6, and [`Sexp`] is the S-expression itself. Both are
//! representation-independent values, which the modules around them read and
//! write in the representations §6 gives.
//!
//! [`encode`] writes an S-expression in the canonical representation of §6.2,
//! the basic transport representation of §6.3, or the advanced representation
//! of §6.4.
//!
//! [RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod base64;

pub mod encode;
pub mod types;

pub use crate::types::{Atom, Sexp};
