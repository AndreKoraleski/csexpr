//! Carrying an S-expression between Rust and Python.
//!
//! A Python value stands for an S-expression without a class of its own. A
//! `list` is a list, `bytes` is an octet string, and [`Atom`] is an octet
//! string that carries a display hint. Text is accepted wherever octets are,
//! and is taken as UTF-8, which §4.6 recommends where the data is text.
//!
//! Neither direction recurses. A list may hold lists as deeply as memory
//! allows, and a conversion that walked them by recursion would exhaust the
//! stack on input that a Python program can build in a loop.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyString, PyTuple};

use csexpr::{Atom, Sexp};

/// Reads an S-expression from a Python value.
///
/// Lists may nest as deeply as [`csexpr::decode::DEFAULT_MAX_DEPTH`], which is
/// what a parser accepts by default. Deeper than that, the tree this builds
/// would be too deep for Rust to drop, so it is refused instead. Nothing a
/// parse returned can reach the limit, since a parse was held to it already.
///
/// # Errors
///
/// Returns [`PyTypeError`] where the value, or anything within it, is neither
/// octets nor a list of them, and [`PyValueError`] where lists nest deeper
/// than the limit.
pub(crate) fn to_sexp(value: &Bound<'_, PyAny>) -> PyResult<Sexp> {
    enum Step<'py> {
        Read(Bound<'py, PyAny>),
        Close,
    }

    let mut work = vec![Step::Read(value.clone())];
    let mut open: Vec<Vec<Sexp>> = Vec::new();

    while let Some(step) = work.pop() {
        let sexp = match step {
            Step::Close => {
                let items = open.pop().ok_or_else(|| {
                    PyTypeError::new_err("a list was closed that was never opened")
                })?;

                Sexp::List(items)
            }
            Step::Read(value) => match items_of(&value)? {
                Some(items) => {
                    if open.len() >= csexpr::decode::DEFAULT_MAX_DEPTH {
                        return Err(PyValueError::new_err(format!(
                            "lists nested deeper than {}",
                            csexpr::decode::DEFAULT_MAX_DEPTH
                        )));
                    }

                    open.push(Vec::with_capacity(items.len()));
                    work.push(Step::Close);
                    work.extend(items.into_iter().rev().map(Step::Read));

                    continue;
                }
                None => Sexp::Atom(to_atom(&value)?),
            },
        };

        match open.last_mut() {
            Some(parent) => parent.push(sexp),
            None => return Ok(sexp),
        }
    }

    Err(PyTypeError::new_err("no S-expression was read"))
}

/// Builds the Python value that stands for an S-expression.
pub(crate) fn from_sexp<'py>(py: Python<'py>, sexp: &Sexp) -> PyResult<Bound<'py, PyAny>> {
    enum Step<'a> {
        Build(&'a Sexp),
        Close(usize),
    }

    let mut work = vec![Step::Build(sexp)];
    let mut built: Vec<Bound<'py, PyAny>> = Vec::new();

    while let Some(step) = work.pop() {
        match step {
            Step::Build(Sexp::Atom(atom)) => built.push(from_atom(py, atom)?),
            Step::Build(Sexp::List(items)) => {
                // The work is a stack, so the elements go on back to front in
                // order to come off front to back.
                work.push(Step::Close(items.len()));
                work.extend(items.iter().rev().map(Step::Build));
            }
            Step::Close(len) => {
                // The elements were built onto the end of the stack, in order,
                // so the last `len` of them are this list's.
                let at = built.len().checked_sub(len).ok_or_else(|| {
                    PyTypeError::new_err("a list was built from elements that are not there")
                })?;

                let items = built.split_off(at);
                let list = PyList::new(py, items)?;

                built.push(list.into_any());
            }
        }
    }

    built
        .pop()
        .ok_or_else(|| PyTypeError::new_err("no value was built"))
}

/// Reads one octet string from a Python value.
///
/// # Errors
///
/// Returns [`PyTypeError`] where the value is neither octets nor text.
pub(crate) fn to_atom(value: &Bound<'_, PyAny>) -> PyResult<Atom> {
    if let Ok(atom) = value.cast::<super::atom::Atom>() {
        return Ok(atom.get().inner().clone());
    }

    Ok(Atom::from(octets_of(value)?))
}

/// Builds the Python value that stands for one octet string.
fn from_atom<'py>(py: Python<'py>, atom: &Atom) -> PyResult<Bound<'py, PyAny>> {
    match atom.hint() {
        // Only a display hint calls for the class. Without one, the octets
        // themselves are the whole of the atom.
        Some(_) => Ok(super::atom::Atom::from_inner(atom.clone())
            .into_pyobject(py)?
            .into_any()),
        None => Ok(PyBytes::new(py, atom.data()).into_any()),
    }
}

/// Reads octets from a Python value, taking text as UTF-8.
///
/// # Errors
///
/// Returns [`PyTypeError`] where the value is neither octets nor text.
pub(crate) fn octets_of(value: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if let Ok(octets) = value.cast::<PyBytes>() {
        return Ok(octets.as_bytes().to_vec());
    }

    if let Ok(text) = value.cast::<PyString>() {
        return Ok(text.to_cow()?.as_bytes().to_vec());
    }

    if let Ok(atom) = value.cast::<super::atom::Atom>() {
        return Ok(atom.get().inner().data().to_vec());
    }

    // bytearray, memoryview, and anything else that lends out octets.
    if let Ok(octets) = value.extract::<Vec<u8>>() {
        return Ok(octets);
    }

    Err(PyTypeError::new_err(format!(
        "expected bytes, str, or Atom, and got {}",
        type_name(value)
    )))
}

/// Returns the elements of a Python value that stands for a list, or nothing
/// where it stands for an octet string.
///
/// A `str` is a sequence in Python and an octet string here, so only a `list`
/// and a `tuple` are read as lists. Anything else that holds elements is
/// turned into one of those by whoever passes it.
fn items_of<'py>(value: &Bound<'py, PyAny>) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
    if let Ok(list) = value.cast::<PyList>() {
        return Ok(Some(list.iter().collect()));
    }

    if let Ok(tuple) = value.cast::<PyTuple>() {
        return Ok(Some(tuple.iter().collect()));
    }

    Ok(None)
}

/// Returns the name of a value's type, for an error that has to say what it
/// was given.
fn type_name(value: &Bound<'_, PyAny>) -> String {
    value
        .get_type()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|_| String::from("a value of an unreadable type"))
}
