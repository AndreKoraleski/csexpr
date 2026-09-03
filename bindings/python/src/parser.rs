//! Reading S-expressions, and the restrictions to read them against.

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::atom::Octets;
use crate::{error, value};

/// A reader of S-expressions, and the restrictions it reads against.
///
/// Built with no arguments it accepts every representation of §6, which is
/// what §6 asks a reader to accept. Each keyword turns on one of the
/// restrictions §8 lists, or bounds how large or how deeply nested the input
/// may be.
///
/// A parser holds no state between parses, so one may be built once and used
/// as often as wanted.
#[pyclass(module = "csexpr", frozen)]
pub(crate) struct Parser {
    inner: csexpr::decode::Parser,
}

#[pymethods]
impl Parser {
    #[new]
    #[pyo3(signature = (
        *,
        canonical = false,
        max_depth = csexpr::decode::DEFAULT_MAX_DEPTH,
        max_atom_len = None,
        max_input_len = None,
        allow_advanced = true,
        allow_transport = true,
        allow_display_hints = true,
        allow_empty_atoms = true,
        allow_empty_lists = true,
        allow_list_as_first_element = true,
        allow_hexadecimal = true,
        allow_base64 = true,
        allow_lengths = true,
    ))]
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
    fn new(
        canonical: bool,
        max_depth: usize,
        max_atom_len: Option<usize>,
        max_input_len: Option<usize>,
        allow_advanced: bool,
        allow_transport: bool,
        allow_display_hints: bool,
        allow_empty_atoms: bool,
        allow_empty_lists: bool,
        allow_list_as_first_element: bool,
        allow_hexadecimal: bool,
        allow_base64: bool,
        allow_lengths: bool,
    ) -> Self {
        let inner = match canonical {
            true => csexpr::decode::Parser::canonical(),
            false => csexpr::decode::Parser::new(),
        };

        Self {
            inner: inner
                .max_depth(max_depth)
                // No limit is how the library says no limit.
                .max_atom_len(max_atom_len.unwrap_or(usize::MAX))
                .max_input_len(max_input_len.unwrap_or(usize::MAX))
                // `canonical` has already turned two of these off, so only a
                // keyword given as false may turn one off again.
                .allow_advanced(allow_advanced && !canonical)
                .allow_transport(allow_transport && !canonical)
                .allow_display_hints(allow_display_hints)
                .allow_empty_atoms(allow_empty_atoms)
                .allow_empty_lists(allow_empty_lists)
                .allow_list_as_first_element(allow_list_as_first_element)
                .allow_hexadecimal(allow_hexadecimal)
                .allow_base64(allow_base64)
                .allow_lengths(allow_lengths),
        }
    }

    /// Reads one S-expression, which is the whole of the input.
    fn parse<'py>(&self, py: Python<'py>, data: Octets) -> PyResult<Bound<'py, PyAny>> {
        let sexp = self.inner.parse(&data.0).map_err(|e| error::raise(py, e))?;

        value::from_sexp(py, &sexp)
    }

    /// Reads one S-expression from the start of the input, and returns it with
    /// the number of octets it occupied.
    ///
    /// What follows is not examined, so this is the way to read a stream of
    /// S-expressions one after another.
    fn parse_prefix<'py>(&self, py: Python<'py>, data: Octets) -> PyResult<Bound<'py, PyTuple>> {
        let (sexp, len) = self
            .inner
            .parse_prefix(&data.0)
            .map_err(|e| error::raise(py, e))?;

        PyTuple::new(
            py,
            [
                value::from_sexp(py, &sexp)?,
                len.into_pyobject(py)?.into_any(),
            ],
        )
    }

    fn __repr__(&self) -> String {
        String::from("Parser()")
    }
}

/// Reads one S-expression, in whichever representation of §6 it is written.
///
/// Build a [`Parser`] where the input comes from somewhere untrusted, or where
/// only one representation is to be accepted.
#[pyfunction]
pub(crate) fn parse<'py>(py: Python<'py>, data: Octets) -> PyResult<Bound<'py, PyAny>> {
    let sexp = csexpr::decode::parse(&data.0).map_err(|e| error::raise(py, e))?;

    value::from_sexp(py, &sexp)
}

/// Reads one S-expression written in the canonical representation of §6.2.
///
/// Read this way where the octets themselves matter, as they do to a signature
/// computed over them.
#[pyfunction]
pub(crate) fn parse_canonical<'py>(py: Python<'py>, data: Octets) -> PyResult<Bound<'py, PyAny>> {
    let sexp = csexpr::decode::parse_canonical(&data.0).map_err(|e| error::raise(py, e))?;

    value::from_sexp(py, &sexp)
}
