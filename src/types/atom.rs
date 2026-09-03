//! S-expression atoms.
//!
//! An S-expression is either an octet string or a list of simpler S-expressions
//! ([RFC 9804], §2).
//!
//! This module provides the octet string as a value: data octets carrying an
//! optional display hint, which describes how the data should be displayed to a
//! user and has no other function (§4.6). If the data is text, §4.6 recommends
//! UTF-8, but interpreting the octets is for the application, so this type
//! neither validates nor converts them.
//!
//! An atom is representation-independent. §6 gives an S-expression three
//! representations: canonical, basic transport, and advanced. Choosing among
//! them belongs to the encoder.
//!
//! §8 lists restrictions an application may impose: forbidding display hints,
//! forbidding zero-length octet strings, capping octet-string size. This type
//! imposes none of them.
//!
//! [RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html

use std::fmt;

/// Display hint for an application that specifies no other default.
///
/// RFC 9804 §4.6 specifies `application/octet-stream` as the display hint an
/// atom without one may be considered to have, in the absence of a default
/// specified by the application. Pass it to [`Atom::effective_hint`] when the
/// application defines no default of its own.
///
/// §10 notes that an atom carrying no display hint may be read by another
/// application under a different default, so an effective hint is a property of
/// the reader rather than of the atom.
pub const DEFAULT_HINT: &[u8] = b"application/octet-stream";

/// An S-expression octet string with an optional display hint.
///
/// The criterion RFC 9804 §4.7 recommends is followed for equality. Two atoms
/// are equal when both their data octets and their display hints are equal. The
/// comparison between two octets is exact, so atoms are case-sensitive.
///
/// §4.7 names two criteria an application may use instead: ignoring the display
/// hint, which is [`eq_ignoring_hint`], and treating an absent hint as the
/// application's default, which [`effective_hint`] supplies. A hint does not
/// nest, as it is composed of octets, not another atom.
///
/// RFC 9804 defines no ordering for octet strings, so an atom has none. An
/// application that needs sorted atoms defines the order it wants over [`data`]
/// and [`hint`].
///
/// An atom is immutable. Changing one means constructing a replacement, which
/// keeps [`Hash`] and [`PartialEq`] stable for atoms held in collections. A
/// list of atoms may still be mutated by replacing its elements.
///
/// [`data`]: Self::data
/// [`hint`]: Self::hint
/// [`eq_ignoring_hint`]: Self::eq_ignoring_hint
/// [`effective_hint`]: Self::effective_hint
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Atom {
    data: Box<[u8]>,
    hint: Option<Box<[u8]>>,
}

impl Atom {
    /// Creates an atom without a display hint.
    ///
    /// RFC 9804 §2 admits an octet string of length zero, and §4.6 allows it to
    /// hold any data representable as octets.
    pub fn new(data: impl AsRef<[u8]>) -> Self {
        Self {
            data: data.as_ref().into(),
            hint: None,
        }
    }

    /// Returns the atom with the given display hint, replacing any it carries.
    ///
    /// RFC 9804 §4.6 precedes an octet string with a single display hint, so an
    /// atom holds at most one. The hint is itself an octet string, and may be
    /// zero length or hold arbitrary octets.
    #[must_use]
    pub fn with_hint(mut self, hint: impl AsRef<[u8]>) -> Self {
        self.hint = Some(hint.as_ref().into());
        self
    }

    /// Returns the data octets.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns the display hint, if present.
    pub fn hint(&self) -> Option<&[u8]> {
        self.hint.as_deref()
    }

    /// Returns the atom's display hint, or `default` if it carries none.
    ///
    /// RFC 9804 §4.7 allows comparing an atom without a display hint as though
    /// it had the default hint for the application.
    ///
    /// Two atoms equal under RFC 9804 §4.7 may still have different canonical
    /// encodings, so it is not suitable for deciding whether signatures will
    /// match.
    pub fn effective_hint<'a>(&'a self, default: &'a [u8]) -> &'a [u8] {
        self.hint().unwrap_or(default)
    }

    /// Returns `true` if two atoms hold the same data octets, whatever their
    /// display hints.
    ///
    /// RFC 9804 §4.7 names ignoring the display hint as a criterion an
    /// application may use in place of the recommended one, which [`PartialEq`]
    /// implements.
    pub fn eq_ignoring_hint(&self, other: &Self) -> bool {
        self.data == other.data
    }

    /// Returns the number of data octets, not counting the display hint.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if there are no data octets.
    ///
    /// RFC 9804 §2 admits a zero-length octet string, so an empty atom is an
    /// ordinary value. A display hint does not make an atom non-empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Consumes the atom and returns its data octets.
    pub fn into_data(self) -> Box<[u8]> {
        self.data
    }

    /// Consumes the atom and returns its data octets and display hint.
    pub fn into_parts(self) -> (Box<[u8]>, Option<Box<[u8]>>) {
        (self.data, self.hint)
    }

    /// Creates an atom from octets the caller already owns.
    ///
    /// This is what [`into_parts`] undoes. [`new`] and [`with_hint`] take
    /// anything that lends octets and so have to copy what they are lent,
    /// which is the right thing where the octets belong to someone else. A
    /// caller holding octets of its own, as a reader does once it has decoded
    /// them, hands them over here instead of having them copied.
    ///
    /// [`into_parts`]: Self::into_parts
    /// [`new`]: Self::new
    /// [`with_hint`]: Self::with_hint
    pub fn from_parts(data: Box<[u8]>, hint: Option<Box<[u8]>>) -> Self {
        Self { data, hint }
    }
}

/// Formats the atom for diagnostics, escaping octets outside printable ASCII.
///
/// The shape resembles a canonical octet string but is not one, as escaping
/// makes the rendered text longer than the lengths it prints.
impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(hint) = self.hint() {
            write!(f, "[{}:{}]", hint.len(), hint.escape_ascii())?;
        }

        write!(f, "{}:{}", self.data.len(), self.data.escape_ascii())
    }
}

impl From<&str> for Atom {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Atom {
    fn from(s: String) -> Self {
        Self::from(s.into_bytes())
    }
}

impl From<&[u8]> for Atom {
    fn from(octets: &[u8]) -> Self {
        Self::new(octets)
    }
}

impl From<Vec<u8>> for Atom {
    fn from(octets: Vec<u8>) -> Self {
        Self {
            data: octets.into(),
            hint: None,
        }
    }
}

impl From<Box<[u8]>> for Atom {
    fn from(octets: Box<[u8]>) -> Self {
        Self {
            data: octets,
            hint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    const DATA: &[u8] = b"data";
    const OTHER_DATA: &[u8] = b"different";
    const HINT: &[u8] = b"hint";
    const OTHER_HINT: &[u8] = b"other";
    const OCTETS: &[u8] = &[0x00, 0xff, 0x1b, 0x7f];

    fn hash_of(atom: &Atom) -> u64 {
        let mut hasher = DefaultHasher::new();
        atom.hash(&mut hasher);
        hasher.finish()
    }

    fn equivalent_under_default(left: &Atom, right: &Atom, default: &[u8]) -> bool {
        left.data() == right.data() && left.effective_hint(default) == right.effective_hint(default)
    }

    // DEFAULT_HINT

    #[test]
    fn default_hint_is_the_media_type_the_rfc_specifies() {
        assert_eq!(DEFAULT_HINT, b"application/octet-stream");
    }

    // Clone, PartialEq, Eq, Hash

    #[test]
    fn eq_holds_when_data_and_hint_are_equal() {
        assert_eq!(Atom::new(DATA), Atom::new(DATA));
        assert_eq!(
            Atom::new(DATA).with_hint(HINT),
            Atom::new(DATA).with_hint(HINT)
        );
    }

    #[test]
    fn eq_fails_when_data_differs() {
        assert_ne!(Atom::new(DATA), Atom::new(OTHER_DATA));
        assert_ne!(
            Atom::new(DATA).with_hint(HINT),
            Atom::new(OTHER_DATA).with_hint(HINT)
        );
    }

    #[test]
    fn eq_fails_when_hint_differs() {
        assert_ne!(
            Atom::new(DATA).with_hint(HINT),
            Atom::new(DATA).with_hint(OTHER_HINT)
        );
    }

    #[test]
    fn eq_distinguishes_an_absent_hint_from_a_present_one() {
        assert_ne!(Atom::new(DATA), Atom::new(DATA).with_hint(HINT));
    }

    #[test]
    fn eq_distinguishes_an_absent_hint_from_an_explicit_default() {
        assert_ne!(Atom::new(DATA), Atom::new(DATA).with_hint(DEFAULT_HINT));
    }

    #[test]
    fn eq_distinguishes_an_absent_hint_from_a_zero_length_one() {
        assert_ne!(Atom::new(DATA), Atom::new(DATA).with_hint(""));
    }

    #[test]
    fn eq_is_case_sensitive_in_data() {
        assert_ne!(Atom::new("data"), Atom::new("DATA"));
    }

    #[test]
    fn eq_is_case_sensitive_in_hint() {
        assert_ne!(
            Atom::new(DATA).with_hint("hint"),
            Atom::new(DATA).with_hint("HINT")
        );
    }

    #[test]
    fn hash_agrees_with_eq() {
        for atom in [
            Atom::new(DATA),
            Atom::new(DATA).with_hint(HINT),
            Atom::new(DATA).with_hint(""),
            Atom::new(""),
        ] {
            assert_eq!(hash_of(&atom), hash_of(&atom.clone()));
        }
    }

    #[test]
    fn clone_equals_the_original() {
        for atom in [
            Atom::new(DATA),
            Atom::new(DATA).with_hint(HINT),
            Atom::new(OCTETS).with_hint(OCTETS),
        ] {
            assert_eq!(atom.clone(), atom);
        }
    }

    // new

    #[test]
    fn new_stores_data() {
        assert_eq!(Atom::new(DATA).data(), DATA);
    }

    #[test]
    fn new_stores_no_hint() {
        assert_eq!(Atom::new(DATA).hint(), None);
    }

    #[test]
    fn new_accepts_zero_length_data() {
        assert_eq!(Atom::new("").data(), b"");
    }

    #[test]
    fn new_accepts_arbitrary_octets() {
        assert_eq!(Atom::new(OCTETS).data(), OCTETS);
    }

    #[test]
    fn new_accepts_data_resembling_syntax() {
        let data: &[u8] = b"()[]{}|#\"3:abc";
        assert_eq!(Atom::new(data).data(), data);
    }

    // with_hint

    #[test]
    fn with_hint_stores_hint() {
        assert_eq!(Atom::new(DATA).with_hint(HINT).hint(), Some(HINT));
    }

    #[test]
    fn with_hint_preserves_data() {
        assert_eq!(Atom::new(DATA).with_hint(HINT).data(), DATA);
    }

    #[test]
    fn with_hint_keeps_at_most_one_hint() {
        let atom = Atom::new(DATA).with_hint(OTHER_HINT).with_hint(HINT);
        assert_eq!(atom.hint(), Some(HINT));
    }

    #[test]
    fn with_hint_accepts_zero_length_hint() {
        assert_eq!(Atom::new(DATA).with_hint("").hint(), Some(&b""[..]));
    }

    #[test]
    fn with_hint_accepts_arbitrary_octets() {
        assert_eq!(Atom::new(DATA).with_hint(OCTETS).hint(), Some(OCTETS));
    }

    // data, hint

    #[test]
    fn data_returns_the_stored_octets() {
        assert_eq!(Atom::new(OCTETS).with_hint(HINT).data(), OCTETS);
    }

    #[test]
    fn hint_returns_none_when_absent() {
        assert_eq!(Atom::new(DATA).hint(), None);
    }

    #[test]
    fn hint_returns_the_stored_octets() {
        assert_eq!(Atom::new(DATA).with_hint(OCTETS).hint(), Some(OCTETS));
    }

    // effective_hint

    #[test]
    fn effective_hint_prefers_the_stored_hint() {
        let atom = Atom::new(DATA).with_hint(HINT);
        assert_eq!(atom.effective_hint(DEFAULT_HINT), HINT);
    }

    #[test]
    fn effective_hint_falls_back_to_the_given_default() {
        assert_eq!(Atom::new(DATA).effective_hint(DEFAULT_HINT), DEFAULT_HINT);
        assert_eq!(Atom::new(DATA).effective_hint(HINT), HINT);
    }

    #[test]
    fn effective_hint_treats_a_zero_length_hint_as_present() {
        assert_eq!(Atom::new(DATA).with_hint("").effective_hint(HINT), b"");
    }

    #[test]
    fn effective_hint_supports_the_default_hint_criterion() {
        let absent = Atom::new(DATA);
        let explicit = Atom::new(DATA).with_hint(DEFAULT_HINT);

        assert_ne!(absent, explicit);
        assert!(equivalent_under_default(&absent, &explicit, DEFAULT_HINT));
        assert!(!equivalent_under_default(
            &absent,
            &Atom::new(DATA).with_hint(HINT),
            DEFAULT_HINT
        ));
    }

    // eq_ignoring_hint

    #[test]
    fn eq_ignoring_hint_holds_across_differing_hints() {
        let hinted = Atom::new(DATA).with_hint(HINT);

        assert!(hinted.eq_ignoring_hint(&Atom::new(DATA).with_hint(OTHER_HINT)));
        assert!(hinted.eq_ignoring_hint(&Atom::new(DATA)));
        assert!(hinted.eq_ignoring_hint(&Atom::new(DATA).with_hint("")));
    }

    #[test]
    fn eq_ignoring_hint_still_compares_data() {
        let atom = Atom::new(DATA).with_hint(HINT);
        assert!(!atom.eq_ignoring_hint(&Atom::new(OTHER_DATA).with_hint(HINT)));
    }

    #[test]
    fn eq_ignoring_hint_is_case_sensitive() {
        assert!(!Atom::new("data").eq_ignoring_hint(&Atom::new("DATA")));
    }

    // len, is_empty

    #[test]
    fn len_counts_data_octets() {
        assert_eq!(Atom::new(DATA).len(), DATA.len());
        assert_eq!(Atom::new("").len(), 0);
    }

    #[test]
    fn len_ignores_hint() {
        assert_eq!(Atom::new(DATA).with_hint(DEFAULT_HINT).len(), DATA.len());
    }

    #[test]
    fn is_empty_reports_zero_length_data() {
        assert!(Atom::new("").is_empty());
        assert!(!Atom::new(DATA).is_empty());
    }

    #[test]
    fn is_empty_ignores_hint() {
        assert!(Atom::new("").with_hint(HINT).is_empty());
    }

    // into_data, into_parts

    #[test]
    fn into_data_returns_the_data_octets() {
        let atom = Atom::new(DATA).with_hint(HINT);
        assert_eq!(&*atom.into_data(), DATA);
    }

    #[test]
    fn into_parts_returns_data_then_hint() {
        let (data, hint) = Atom::new(DATA).with_hint(HINT).into_parts();

        assert_eq!(&*data, DATA);
        assert_eq!(hint.as_deref(), Some(HINT));
    }

    #[test]
    fn into_parts_round_trips_every_hint_state() {
        for atom in [
            Atom::new(DATA),
            Atom::new(DATA).with_hint(HINT),
            Atom::new(DATA).with_hint(""),
            Atom::new("").with_hint(HINT),
            Atom::new(OCTETS).with_hint(OCTETS),
        ] {
            let (data, hint) = atom.clone().into_parts();
            let rebuilt = match hint {
                Some(hint) => Atom::from(data).with_hint(hint),
                None => Atom::from(data),
            };

            assert_eq!(rebuilt, atom);
        }
    }

    // from_parts

    #[test]
    fn from_parts_undoes_into_parts() {
        for atom in [
            Atom::new(DATA),
            Atom::new(DATA).with_hint(HINT),
            Atom::new(DATA).with_hint(""),
            Atom::new("").with_hint(HINT),
            Atom::new("").with_hint(""),
            Atom::new(OCTETS).with_hint(OCTETS),
        ] {
            let (data, hint) = atom.clone().into_parts();

            assert_eq!(Atom::from_parts(data, hint), atom);
        }
    }

    #[test]
    fn from_parts_builds_what_new_and_with_hint_build() {
        let data: Box<[u8]> = Box::from(DATA);
        let hint: Box<[u8]> = Box::from(HINT);

        assert_eq!(Atom::from_parts(data.clone(), None), Atom::new(DATA));
        assert_eq!(
            Atom::from_parts(data, Some(hint)),
            Atom::new(DATA).with_hint(HINT)
        );
    }

    #[test]
    fn from_parts_distinguishes_no_hint_from_a_zero_length_one() {
        let data: Box<[u8]> = Box::from(DATA);
        let absent = Atom::from_parts(data.clone(), None);
        let empty = Atom::from_parts(data, Some(Box::from(&b""[..])));

        assert_ne!(absent, empty);
        assert_eq!(absent.hint(), None);
        assert_eq!(empty.hint(), Some(&b""[..]));
    }

    // Debug

    #[test]
    fn debug_renders_data() {
        assert_eq!(format!("{:?}", Atom::new(DATA)), "4:data");
    }

    #[test]
    fn debug_renders_hint_before_data() {
        let atom = Atom::new(DATA).with_hint(HINT);
        assert_eq!(format!("{atom:?}"), "[4:hint]4:data");
    }

    #[test]
    fn debug_renders_zero_length_data_and_hint() {
        assert_eq!(format!("{:?}", Atom::new("")), "0:");
        assert_eq!(format!("{:?}", Atom::new("").with_hint("")), "[0:]0:");
    }

    #[test]
    fn debug_escapes_non_printable_octets() {
        let atom = Atom::new([0xff, 0x00, b'\n', b'\t']);
        assert_eq!(format!("{atom:?}"), r"4:\xff\x00\n\t");
    }

    #[test]
    fn debug_escapes_quotes_and_backslash() {
        let atom = Atom::new(br#""'\"#);
        assert_eq!(format!("{atom:?}"), r#"3:\"\'\\"#);
    }

    #[test]
    fn debug_is_lossless() {
        let escaped = format!("{:?}", Atom::new(br"\xff"));
        let raw = format!("{:?}", Atom::new([0xff]));

        assert_ne!(escaped, raw);
    }

    // From

    #[test]
    fn from_conversions_agree() {
        let expected = Atom::new(DATA);

        assert_eq!(Atom::from("data"), expected);
        assert_eq!(Atom::from(String::from("data")), expected);
        assert_eq!(Atom::from(DATA), expected);
        assert_eq!(Atom::from(DATA.to_vec()), expected);
        assert_eq!(Atom::from(Box::from(DATA)), expected);
    }

    #[test]
    fn from_conversions_store_no_hint() {
        assert_eq!(Atom::from("data").hint(), None);
        assert_eq!(Atom::from(String::from("data")).hint(), None);
        assert_eq!(Atom::from(DATA).hint(), None);
        assert_eq!(Atom::from(DATA.to_vec()).hint(), None);
        assert_eq!(Atom::from(Box::from(DATA)).hint(), None);
    }

    #[test]
    fn from_conversions_accept_arbitrary_octets() {
        assert_eq!(Atom::from(OCTETS).data(), OCTETS);
        assert_eq!(Atom::from(OCTETS.to_vec()).data(), OCTETS);
        assert_eq!(Atom::from(Box::from(OCTETS)).data(), OCTETS);
    }

    #[test]
    fn from_conversions_accept_zero_length_data() {
        assert!(Atom::from("").is_empty());
        assert!(Atom::from(Vec::new()).is_empty());
    }
}
