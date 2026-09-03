//! Whether the canonical representation is as unique as RFC 9804 §6.2 says.
//!
//! §6.2 gives one S-expression exactly one canonical representation, which is
//! what makes a signature over those octets mean anything. Turned around, that
//! says any octets the canonical reader accepts have to be exactly the octets
//! the canonical writer would have produced for the value it read. Anything
//! else would be a second spelling of a signed value.

#![no_main]

use csexpr::decode;
use csexpr::encode::canonical;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|octets: &[u8]| {
    let Ok(sexp) = decode::parse_canonical(octets) else {
        return;
    };

    assert_eq!(canonical::to_vec(&sexp), octets);
});
