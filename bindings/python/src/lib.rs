//! Python bindings for [`csexpr`].
//!
//! The Rust library is the whole of what is bound here. This crate holds only
//! the conversions between a Python value and a [`csexpr::Sexp`], and the
//! classes and functions that carry them across.

use pyo3::prelude::*;

mod atom;
mod error;
mod parser;
mod value;

/// Builds the extension module the `csexpr` package imports.
#[pymodule]
fn _csexpr(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<atom::Atom>()?;
    module.add_class::<parser::Parser>()?;
    module.add("ParseError", module.py().get_type::<error::ParseError>())?;
    module.add("DEFAULT_HINT", csexpr::types::DEFAULT_HINT)?;
    module.add("DEFAULT_MAX_DEPTH", csexpr::decode::DEFAULT_MAX_DEPTH)?;
    module.add_function(wrap_pyfunction!(parser::parse, module)?)?;
    module.add_function(wrap_pyfunction!(parser::parse_canonical, module)?)?;
    module.add_function(wrap_pyfunction!(to_canonical, module)?)?;
    module.add_function(wrap_pyfunction!(to_transport, module)?)?;
    module.add_function(wrap_pyfunction!(to_advanced, module)?)?;

    Ok(())
}

/// Returns the canonical representation of the S-expression (§6.2).
#[pyfunction]
fn to_canonical(py: Python<'_>, sexp: &Bound<'_, PyAny>) -> PyResult<Py<pyo3::types::PyBytes>> {
    let sexp = value::to_sexp(sexp)?;
    let octets = csexpr::encode::canonical::to_vec(&sexp);

    Ok(pyo3::types::PyBytes::new(py, &octets).unbind())
}

/// Returns the basic transport representation of the S-expression (§6.3).
#[pyfunction]
fn to_transport(sexp: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(csexpr::encode::transport::to_string(&value::to_sexp(sexp)?))
}

/// Returns the advanced representation of the S-expression (§6.4).
#[pyfunction]
fn to_advanced(sexp: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(csexpr::encode::advanced::to_string(&value::to_sexp(sexp)?))
}
