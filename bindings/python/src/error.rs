//! What a parse raises when it stops.

use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

create_exception!(
    csexpr,
    ParseError,
    PyValueError,
    "Raised where input is not an S-expression the parser accepts.\n\n\
     Carries `offset`, the octet the offending construct begins at, and \
     `kind`, a short name for what was wrong."
);

/// Turns a failure to parse into the exception that stands for it.
///
/// The offset and the kind are set as attributes rather than left inside the
/// message, so that a caller can act on either without reading English.
pub(crate) fn raise(py: Python<'_>, error: csexpr::decode::Error) -> PyErr {
    let raised = ParseError::new_err(error.to_string());
    let value = raised.value(py);

    // Nothing here should fail on an exception just built. Whatever did is
    // worth raising in place of what it was describing.
    if let Err(failure) = value.setattr("offset", error.offset()) {
        return failure;
    }

    if let Err(failure) = value.setattr("kind", kind_of(error.kind())) {
        return failure;
    }

    raised
}

/// Returns the name of an error kind, as Python spells names.
fn kind_of(kind: csexpr::decode::ErrorKind) -> String {
    let mut name = String::new();

    // The Rust name is written in camel case, and reads here the way a Python
    // constant does.
    for character in format!("{kind:?}").chars() {
        if character.is_ascii_uppercase() && !name.is_empty() {
            name.push('_');
        }

        name.push(character.to_ascii_lowercase());
    }

    name
}
