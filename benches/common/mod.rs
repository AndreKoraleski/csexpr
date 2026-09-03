//! S-expressions to measure against.
//!
//! Each one stands for a shape that shows up in practice rather than a shape
//! chosen to flatter a number. Every benchmark compiles this file for itself,
//! so what one does not use is left alone rather than reported.

#![allow(dead_code)]

use csexpr::{Atom, Sexp, sexp};

/// A certificate, of the shape SPKI and SDSI put in one.
///
/// Mostly short tokens, nested a few deep, with one atom of raw octets where
/// a hash would be.
pub fn certificate() -> Sexp {
    sexp![
        "cert",
        sexp!["issuer", sexp!["name", "bob"]],
        sexp!["subject", sexp!["hash", "sha256", Atom::new([0xab; 32])]],
        sexp!["not-before", "2026-01-01_00:00:00"],
        sexp!["not-after", "2027-01-01_00:00:00"],
        sexp!["tag", sexp!["ftp", "ftp.example.com", "/pub"]],
    ]
}

/// Twenty octet strings, every one of them carrying a display hint.
///
/// A display hint is an octet string of its own, so this is the shape that
/// says what carrying one costs.
pub fn hinted() -> Sexp {
    Sexp::list((0..20).map(|index| {
        let atom = Atom::new(format!("value number {index}")).with_hint("text/plain");

        Sexp::from(atom)
    }))
}

/// One list of a thousand short octet strings, and nothing nested.
pub fn wide() -> Sexp {
    Sexp::list((0..1000).map(|index| Sexp::atom(format!("{index}"))))
}

/// One octet string of sixty-four kilobytes.
pub fn large_atom() -> Sexp {
    Sexp::atom(vec![0x5a; 64 * 1024])
}

/// Lists nested as deeply as a parser accepts without being told otherwise.
///
/// Reading and writing both walk an explicit stack rather than recursing, and
/// this is the shape that says what that costs. It is built at the limit, so
/// that a parser reading it back is working as hard as it will be asked to.
pub fn deep() -> Sexp {
    let mut sexp = Sexp::atom("data");

    for _ in 0..csexpr::decode::DEFAULT_MAX_DEPTH {
        sexp = Sexp::list([sexp]);
    }

    sexp
}
