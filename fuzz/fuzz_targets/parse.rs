//! Reading arbitrary octets, and writing back whatever they turned out to be.
//!
//! Two things are asserted here. Reading never panics, whatever it is given,
//! and anything that was read survives being written and read again in each of
//! the three representations of RFC 9804 §6.

#![no_main]

use csexpr::decode;
use csexpr::encode::{advanced, canonical, transport};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|octets: &[u8]| {
    let Ok(sexp) = decode::parse(octets) else {
        return;
    };

    // The canonical representation is the one that gets signed, so the value
    // has to come back from it unchanged.
    let written = canonical::to_vec(&sexp);
    assert_eq!(decode::parse_canonical(&written).unwrap(), sexp);

    // The other two representations carry the same value by other means.
    assert_eq!(decode::parse(&transport::to_vec(&sexp)).unwrap(), sexp);
    assert_eq!(
        decode::parse(advanced::to_string(&sexp).as_bytes()).unwrap(),
        sexp
    );
});
