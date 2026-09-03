//! An octet string carrying a display hint, as a Python class.
//!
//! A Python value stands for an S-expression directly. `bytes` is an octet
//! string, a `list` is a list, and this class is the one case the two cannot
//! express, which is an octet string that carries a display hint (§4.6).

use pyo3::basic::CompareOp;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::{Borrowed, PyErr};

/// An octet string with a display hint.
///
/// Building one is the only way to give an octet string a hint, since plain
/// `bytes` carries none. Text is taken as UTF-8, which §4.6 recommends where
/// the data is text.
#[pyclass(module = "csexpr", frozen)]
pub(crate) struct Atom {
    inner: csexpr::Atom,
}

impl Atom {
    /// Wraps an atom that already carries a hint.
    pub(crate) fn from_inner(inner: csexpr::Atom) -> Self {
        Self { inner }
    }

    /// Returns the atom this class stands for.
    pub(crate) fn inner(&self) -> &csexpr::Atom {
        &self.inner
    }
}

#[pymethods]
impl Atom {
    /// Creates an octet string, with a display hint if one is given.
    #[new]
    #[pyo3(signature = (data, hint = None))]
    fn new(data: Octets, hint: Option<Octets>) -> Self {
        // The octets were built here, so they are handed over rather than
        // lent and copied.
        let hint = hint.map(|hint| hint.0.into());

        Self {
            inner: csexpr::Atom::from_parts(data.0.into(), hint),
        }
    }

    /// The data octets.
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.inner.data())
    }

    /// The display hint, or `None` where the atom carries none.
    #[getter]
    fn hint<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        self.inner.hint().map(|hint| PyBytes::new(py, hint))
    }

    /// Returns the display hint, or `default` where the atom carries none.
    ///
    /// §4.7 allows an atom without a display hint to be compared as though it
    /// had whichever hint the application supplies by default.
    fn effective_hint<'py>(&self, py: Python<'py>, default: Octets) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.inner.effective_hint(&default.0))
    }

    /// Returns whether two octet strings hold the same data, whatever their
    /// display hints.
    ///
    /// §4.7 names this as a criterion an application may use in place of the
    /// one `==` implements.
    fn eq_ignoring_hint(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let other = super::value::to_atom(other)?;

        Ok(self.inner.eq_ignoring_hint(&other))
    }

    /// The number of data octets, not counting the display hint.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Compares two atoms by the criterion §4.7 recommends, which is equal
    /// data and equal display hints.
    ///
    /// An atom carrying no hint is equal to the `bytes` holding the same
    /// octets, since that is what `bytes` stands for.
    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<Py<PyAny>> {
        let py = other.py();

        let equal = match super::value::to_atom(other) {
            Ok(other) => self.inner == other,
            // Anything an octet string cannot be read from is unequal rather
            // than an error, as Python asks of a comparison.
            Err(_) => false,
        };

        match op {
            CompareOp::Eq => Ok(equal.into_pyobject(py)?.to_owned().unbind().into()),
            CompareOp::Ne => Ok((!equal).into_pyobject(py)?.to_owned().unbind().into()),
            // RFC 9804 defines no ordering for octet strings, so this type has
            // none.
            _ => Ok(py.NotImplemented()),
        }
    }

    /// Hashes the data and the display hint together, agreeing with `==`.
    fn __hash__(&self) -> u64 {
        use std::hash::{DefaultHasher, Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.inner.hash(&mut hasher);

        hasher.finish()
    }

    fn __repr__(&self) -> String {
        match self.inner.hint() {
            Some(hint) => format!(
                "Atom({}, hint={})",
                escaped(self.inner.data()),
                escaped(hint)
            ),
            None => format!("Atom({})", escaped(self.inner.data())),
        }
    }
}

/// Renders octets the way Python renders a `bytes` literal.
fn escaped(octets: &[u8]) -> String {
    format!("b\"{}\"", octets.escape_ascii())
}

/// Octets taken from Python, which may be given as `bytes` or as text.
pub(crate) struct Octets(pub(crate) Vec<u8>);
impl<'a, 'py> FromPyObject<'a, 'py> for Octets {
    type Error = PyErr;

    fn extract(value: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        super::value::octets_of(&value).map(Self)
    }
}
