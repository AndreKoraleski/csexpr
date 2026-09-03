//! The values an S-expression is made of.
//!
//! RFC 9804 §2 admits two of them. An octet string is [`Atom`], and an octet
//! string or a list of simpler S-expressions is [`Sexp`]. Both are
//! representation-independent, so neither carries any trace of the
//! representation it was read from or will be written in.

mod atom;
mod sexp;

pub use atom::{Atom, DEFAULT_HINT};
pub use sexp::{Preorder, Sexp};
