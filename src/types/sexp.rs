//! S-expressions.
//!
//! An S-expression is either an octet string or a list of simpler
//! S-expressions ([RFC 9804], §2). This module provides that value as [`Sexp`],
//! pairing the octet string of [`Atom`] with a list holding S-expressions of
//! either kind.
//!
//! §5 gives a list no meaning of its own. It is an ordered sequence, it may be
//! empty, and whatever its position or its first element signifies belongs to
//! the application. Nothing here interprets one.
//!
//! A `Sexp` is representation-independent, as an [`Atom`] is. §6 gives an
//! S-expression a canonical, a basic transport, and an advanced
//! representation. Writing them is [`crate::encode`] and reading them is
//! [`crate::decode`].
//!
//! §8 lists restrictions an application may impose, among them forbidding
//! empty lists and forbidding a list whose first element is a list. This type
//! imposes none of them, and [`crate::decode::Parser`] applies them to input.
//!
//! [RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html

use std::fmt;
use std::iter::FusedIterator;

use crate::types::Atom;

/// An S-expression, either an octet string or a list of S-expressions.
///
/// RFC 9804 §2 admits exactly these two cases, so the enum is closed and both
/// variants are public. Matching one by value takes it apart, [`as_atom`] and
/// [`as_list`] borrow instead, and [`into_atom`] and [`into_list`] keep one
/// case and discard the other.
///
/// Two S-expressions are equal when they are the same case and their contents
/// are equal, which for atoms is the criterion §4.7 recommends and [`Atom`]
/// implements. Lists compare element by element, in order. An atom is never
/// equal to a list, not even to a list holding only that atom.
///
/// RFC 9804 defines no ordering for S-expressions, so this type has none. An
/// application that needs sorted S-expressions defines the order it wants.
///
/// # Depth
///
/// A list may hold lists without limit, and [`Clone`], [`PartialEq`], [`Hash`]
/// and dropping each recurse once per level of nesting. Input parsed by
/// [`crate::decode::Parser`] is bounded by its depth restriction, which
/// defaults to a depth the stack can hold. An S-expression built by hand, or
/// parsed with that restriction raised far above its default, can nest deeply
/// enough to exhaust the stack under those operations. Encoding does not
/// recurse, nor do [`depth`] and [`preorder`].
///
/// [`as_atom`]: Self::as_atom
/// [`as_list`]: Self::as_list
/// [`into_atom`]: Self::into_atom
/// [`into_list`]: Self::into_list
/// [`depth`]: Self::depth
/// [`preorder`]: Self::preorder
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Sexp {
    /// An octet string, carrying an optional display hint (§4.6).
    Atom(Atom),
    /// A list of S-expressions, possibly empty (§5).
    List(Vec<Sexp>),
}

impl Sexp {
    /// Creates an atom without a display hint.
    ///
    /// This is [`Atom::new`] lifted into an S-expression. To give an atom a
    /// display hint, build it with [`Atom::with_hint`] and convert the result,
    /// which [`From<Atom>`] does.
    ///
    /// [`From<Atom>`]: Self::from
    pub fn atom(data: impl AsRef<[u8]>) -> Self {
        Self::Atom(Atom::new(data))
    }

    /// Creates a list holding the given S-expressions, in order.
    ///
    /// RFC 9804 §5 admits a list of length zero, so an empty iterator yields
    /// the empty list rather than nothing.
    pub fn list(items: impl IntoIterator<Item = Sexp>) -> Self {
        Self::List(items.into_iter().collect())
    }

    /// Returns `true` if this is an atom.
    pub fn is_atom(&self) -> bool {
        matches!(self, Self::Atom(_))
    }

    /// Returns `true` if this is a list.
    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    /// Returns the atom, or `None` if this is a list.
    pub fn as_atom(&self) -> Option<&Atom> {
        match self {
            Self::Atom(atom) => Some(atom),
            Self::List(_) => None,
        }
    }

    /// Returns the list's elements, or `None` if this is an atom.
    ///
    /// The empty list yields an empty slice, which is distinct from the `None`
    /// an atom yields.
    pub fn as_list(&self) -> Option<&[Sexp]> {
        match self {
            Self::Atom(_) => None,
            Self::List(items) => Some(items),
        }
    }

    /// Returns the list's elements for modification, or `None` if this is an
    /// atom.
    ///
    /// An [`Atom`] is immutable, so a tree changes by replacing the elements
    /// of the lists that hold them.
    pub fn as_list_mut(&mut self) -> Option<&mut Vec<Sexp>> {
        match self {
            Self::Atom(_) => None,
            Self::List(items) => Some(items),
        }
    }

    /// Consumes the S-expression and returns the atom, or `None` if it is a
    /// list.
    pub fn into_atom(self) -> Option<Atom> {
        match self {
            Self::Atom(atom) => Some(atom),
            Self::List(_) => None,
        }
    }

    /// Consumes the S-expression and returns the list's elements, or `None` if
    /// it is an atom.
    pub fn into_list(self) -> Option<Vec<Sexp>> {
        match self {
            Self::Atom(_) => None,
            Self::List(items) => Some(items),
        }
    }

    /// Returns the element at `index`, or `None` if this is an atom or the
    /// list is no longer than `index`.
    ///
    /// An atom holds no elements rather than being an element of itself, so
    /// indexing one yields `None` at every index.
    pub fn get(&self, index: usize) -> Option<&Sexp> {
        self.as_list()?.get(index)
    }

    /// Returns how deeply lists nest, counting the outermost list as one.
    ///
    /// An atom has depth zero, `()` and `(a b)` have depth one, and `((a))`
    /// has depth two. This is the quantity [`crate::decode::Parser`] bounds
    /// while parsing, so an S-expression that came from a parser nests no
    /// deeper than the restriction that parser was given.
    ///
    /// The traversal is iterative, so it does not consume stack in proportion
    /// to the depth it reports.
    pub fn depth(&self) -> usize {
        let mut deepest = 0;
        let mut stack = vec![(self, 0usize)];

        while let Some((node, enclosing)) = stack.pop() {
            if let Self::List(items) = node {
                let depth = enclosing + 1;
                deepest = deepest.max(depth);
                stack.extend(items.iter().map(|item| (item, depth)));
            }
        }

        deepest
    }

    /// Returns an iterator over this S-expression and every S-expression
    /// within it, in depth-first prefix order.
    ///
    /// The first item is always the S-expression itself, so an atom yields one
    /// item, and a list yields itself before its elements and each element
    /// before that element's own. This is the order in which the parts appear
    /// in every representation of §6.
    ///
    /// The traversal is iterative, so it does not consume stack in proportion
    /// to the depth it visits.
    pub fn preorder(&self) -> Preorder<'_> {
        Preorder { stack: vec![self] }
    }
}

/// Iterator over an S-expression and everything within it, in depth-first
/// prefix order.
///
/// Returned by [`Sexp::preorder`].
#[derive(Clone, Debug)]
pub struct Preorder<'a> {
    stack: Vec<&'a Sexp>,
}

impl<'a> Iterator for Preorder<'a> {
    type Item = &'a Sexp;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;

        if let Sexp::List(items) = node {
            self.stack.extend(items.iter().rev());
        }

        Some(node)
    }
}

impl FusedIterator for Preorder<'_> {}

/// Formats the S-expression for diagnostics, each atom as [`Atom`] formats it.
///
/// The shape resembles the canonical representation of §6.2 but is not one, as
/// escaping makes the rendered text longer than the lengths it prints. Write
/// the canonical representation with [`crate::encode::canonical`].
impl fmt::Debug for Sexp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        enum Step<'a> {
            Node(&'a Sexp),
            Close,
        }

        let mut stack = vec![Step::Node(self)];

        while let Some(step) = stack.pop() {
            match step {
                Step::Close => f.write_str(")")?,
                Step::Node(Sexp::Atom(atom)) => write!(f, "{atom:?}")?,
                Step::Node(Sexp::List(items)) => {
                    f.write_str("(")?;
                    stack.push(Step::Close);
                    stack.extend(items.iter().rev().map(Step::Node));
                }
            }
        }

        Ok(())
    }
}

impl From<Atom> for Sexp {
    fn from(atom: Atom) -> Self {
        Self::Atom(atom)
    }
}

impl From<&str> for Sexp {
    fn from(s: &str) -> Self {
        Self::Atom(Atom::from(s))
    }
}

impl From<String> for Sexp {
    fn from(s: String) -> Self {
        Self::Atom(Atom::from(s))
    }
}

impl From<&[u8]> for Sexp {
    fn from(octets: &[u8]) -> Self {
        Self::Atom(Atom::from(octets))
    }
}

impl From<Vec<u8>> for Sexp {
    fn from(octets: Vec<u8>) -> Self {
        Self::Atom(Atom::from(octets))
    }
}

impl From<Box<[u8]>> for Sexp {
    fn from(octets: Box<[u8]>) -> Self {
        Self::Atom(Atom::from(octets))
    }
}

impl From<Vec<Sexp>> for Sexp {
    fn from(items: Vec<Sexp>) -> Self {
        Self::List(items)
    }
}

impl<const N: usize> From<[Sexp; N]> for Sexp {
    fn from(items: [Sexp; N]) -> Self {
        Self::List(items.into())
    }
}

/// Collects S-expressions into a list, as [`Sexp::list`] does.
impl FromIterator<Sexp> for Sexp {
    fn from_iter<I: IntoIterator<Item = Sexp>>(items: I) -> Self {
        Self::list(items)
    }
}

/// Appends to a list, and replaces an atom with a list of what is appended.
///
/// An atom holds no elements, so there is nothing to append to. Extending one
/// discards it, which keeps the operation total. Test with [`Sexp::is_list`]
/// first where that would hide a mistake.
impl Extend<Sexp> for Sexp {
    fn extend<I: IntoIterator<Item = Sexp>>(&mut self, items: I) {
        match self {
            Self::List(existing) => existing.extend(items),
            Self::Atom(_) => *self = Self::list(items),
        }
    }
}

/// Builds a list from values convertible into S-expressions.
///
/// Every element passes through [`From`], so an octet string in any form
/// [`Sexp`] converts from becomes an atom, and an S-expression already built,
/// by a nested invocation or from a hinted [`Atom`], is taken as it stands.
/// The macro always builds a list, including from no elements at all, which
/// RFC 9804 §5 admits.
///
/// # Examples
///
/// ```
/// use sexp::{Sexp, sexp};
///
/// let cert = sexp!["issuer", sexp!["name", "bob"], sexp![]];
///
/// assert_eq!(cert.depth(), 2);
/// assert_eq!(cert.get(0), Some(&Sexp::atom("issuer")));
/// assert_eq!(cert.get(2), Some(&Sexp::list([])));
/// ```
#[macro_export]
macro_rules! sexp {
    ($($item:expr),* $(,)?) => {
        $crate::types::Sexp::list([$($crate::types::Sexp::from($item)),*])
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    const DATA: &[u8] = b"data";
    const HINT: &[u8] = b"hint";
    const OCTETS: &[u8] = &[0x00, 0xff, 0x1b, 0x7f];

    /// A nesting deeper than any stack the tests may assume.
    const DEEP: usize = 100_000;

    fn hash_of(sexp: &Sexp) -> u64 {
        let mut hasher = DefaultHasher::new();
        sexp.hash(&mut hasher);
        hasher.finish()
    }

    /// Builds `depth` nested lists around one atom, without recursing.
    fn nested(depth: usize) -> Sexp {
        let mut sexp = Sexp::atom(DATA);

        for _ in 0..depth {
            sexp = Sexp::list([sexp]);
        }

        sexp
    }

    /// Drops an S-expression without recursing, by lifting every list's
    /// elements out before the list that held them is dropped.
    ///
    /// Dropping a `Sexp` recurses once per level of nesting, which the type
    /// documents, so a test that builds something deeper than the stack can
    /// hold has to take it apart this way.
    fn dismantle(sexp: Sexp) {
        let mut stack = vec![sexp];

        while let Some(node) = stack.pop() {
            if let Some(items) = node.into_list() {
                stack.extend(items);
            }
        }
    }

    // atom, list

    #[test]
    fn atom_holds_the_data_octets() {
        let sexp = Sexp::atom(DATA);

        assert_eq!(sexp.as_atom(), Some(&Atom::new(DATA)));
        assert_eq!(sexp.as_atom().map(Atom::data), Some(DATA));
    }

    #[test]
    fn atom_stores_no_hint() {
        assert_eq!(Sexp::atom(DATA).as_atom().and_then(Atom::hint), None);
    }

    #[test]
    fn atom_accepts_zero_length_data() {
        assert_eq!(Sexp::atom(""), Sexp::Atom(Atom::new("")));
    }

    #[test]
    fn atom_accepts_arbitrary_octets() {
        assert_eq!(Sexp::atom(OCTETS).as_atom().map(Atom::data), Some(OCTETS));
    }

    #[test]
    fn list_holds_its_elements_in_order() {
        let sexp = Sexp::list([Sexp::atom("a"), Sexp::atom("b")]);
        let expected = [Sexp::atom("a"), Sexp::atom("b")];

        assert_eq!(sexp.as_list(), Some(&expected[..]));
    }

    #[test]
    fn list_accepts_no_elements() {
        assert_eq!(Sexp::list([]).as_list(), Some(&[][..]));
    }

    #[test]
    fn list_accepts_lists_as_elements() {
        let sexp = Sexp::list([Sexp::list([Sexp::atom(DATA)])]);

        assert_eq!(sexp.get(0), Some(&Sexp::list([Sexp::atom(DATA)])));
    }

    // is_atom, is_list

    #[test]
    fn is_atom_and_is_list_distinguish_the_two_cases() {
        let atom = Sexp::atom(DATA);
        let list = Sexp::list([]);

        assert!(atom.is_atom() && !atom.is_list());
        assert!(list.is_list() && !list.is_atom());
    }

    // as_atom, as_list, as_list_mut

    #[test]
    fn as_atom_returns_none_for_a_list() {
        assert_eq!(Sexp::list([Sexp::atom(DATA)]).as_atom(), None);
    }

    #[test]
    fn as_list_returns_none_for_an_atom() {
        assert_eq!(Sexp::atom(DATA).as_list(), None);
    }

    #[test]
    fn as_list_distinguishes_the_empty_list_from_an_empty_atom() {
        assert_eq!(Sexp::list([]).as_list(), Some(&[][..]));
        assert_eq!(Sexp::atom("").as_list(), None);
        assert_ne!(Sexp::list([]), Sexp::atom(""));
    }

    #[test]
    fn as_list_mut_replaces_elements() {
        let mut sexp = Sexp::list([Sexp::atom("a")]);

        sexp.as_list_mut().unwrap()[0] = Sexp::atom("b");

        assert_eq!(sexp, Sexp::list([Sexp::atom("b")]));
    }

    #[test]
    fn as_list_mut_returns_none_for_an_atom() {
        assert!(Sexp::atom(DATA).as_list_mut().is_none());
    }

    // into_atom, into_list

    #[test]
    fn into_atom_returns_the_atom() {
        let atom = Atom::new(DATA).with_hint(HINT);

        assert_eq!(Sexp::Atom(atom.clone()).into_atom(), Some(atom));
    }

    #[test]
    fn into_atom_returns_none_for_a_list() {
        assert_eq!(Sexp::list([]).into_atom(), None);
    }

    #[test]
    fn into_list_returns_the_elements() {
        let sexp = Sexp::list([Sexp::atom("a"), Sexp::atom("b")]);

        assert_eq!(
            sexp.into_list(),
            Some(vec![Sexp::atom("a"), Sexp::atom("b")])
        );
    }

    #[test]
    fn into_list_returns_none_for_an_atom() {
        assert_eq!(Sexp::atom(DATA).into_list(), None);
    }

    // get

    #[test]
    fn get_returns_the_element_at_the_index() {
        let sexp = Sexp::list([Sexp::atom("a"), Sexp::atom("b")]);

        assert_eq!(sexp.get(0), Some(&Sexp::atom("a")));
        assert_eq!(sexp.get(1), Some(&Sexp::atom("b")));
    }

    #[test]
    fn get_returns_none_past_the_end() {
        assert_eq!(Sexp::list([Sexp::atom("a")]).get(1), None);
        assert_eq!(Sexp::list([]).get(0), None);
    }

    #[test]
    fn get_returns_none_for_an_atom_at_every_index() {
        let atom = Sexp::atom(DATA);

        assert_eq!(atom.get(0), None);
        assert_eq!(atom.get(usize::MAX), None);
    }

    // depth

    #[test]
    fn depth_of_an_atom_is_zero() {
        assert_eq!(Sexp::atom(DATA).depth(), 0);
    }

    #[test]
    fn depth_counts_the_outermost_list_as_one() {
        assert_eq!(Sexp::list([]).depth(), 1);
        assert_eq!(Sexp::list([Sexp::atom("a"), Sexp::atom("b")]).depth(), 1);
    }

    #[test]
    fn depth_counts_nesting() {
        assert_eq!(nested(2).depth(), 2);
        assert_eq!(nested(37).depth(), 37);
    }

    #[test]
    fn depth_reports_the_deepest_branch() {
        let sexp = Sexp::list([Sexp::atom("a"), nested(4)]);

        assert_eq!(sexp.depth(), 5);
    }

    #[test]
    fn depth_does_not_recurse() {
        let deep = nested(DEEP);

        assert_eq!(deep.depth(), DEEP);

        dismantle(deep);
    }

    // preorder

    #[test]
    fn preorder_yields_an_atom_alone() {
        let atom = Sexp::atom(DATA);
        let visited: Vec<_> = atom.preorder().collect();

        assert_eq!(visited, vec![&atom]);
    }

    #[test]
    fn preorder_yields_a_list_before_its_elements() {
        let sexp = Sexp::list([Sexp::atom("a"), Sexp::list([Sexp::atom("b")])]);
        let visited: Vec<_> = sexp.preorder().collect();

        assert_eq!(visited.len(), 4);
        assert_eq!(visited[0], &sexp);
        assert_eq!(visited[1], &Sexp::atom("a"));
        assert_eq!(visited[2], &Sexp::list([Sexp::atom("b")]));
        assert_eq!(visited[3], &Sexp::atom("b"));
    }

    #[test]
    fn preorder_yields_the_empty_list_alone() {
        assert_eq!(Sexp::list([]).preorder().count(), 1);
    }

    #[test]
    fn preorder_is_fused() {
        let sexp = Sexp::atom(DATA);
        let mut visited = sexp.preorder();

        assert!(visited.next().is_some());
        assert!(visited.next().is_none());
        assert!(visited.next().is_none());
    }

    #[test]
    fn preorder_does_not_recurse() {
        let deep = nested(DEEP);

        assert_eq!(deep.preorder().count(), DEEP + 1);

        dismantle(deep);
    }

    // Clone, PartialEq, Eq, Hash

    #[test]
    fn eq_holds_for_equal_trees() {
        assert_eq!(Sexp::atom(DATA), Sexp::atom(DATA));
        assert_eq!(
            Sexp::list([Sexp::atom(DATA)]),
            Sexp::list([Sexp::atom(DATA)])
        );
    }

    #[test]
    fn eq_fails_across_the_two_cases() {
        assert_ne!(Sexp::atom(DATA), Sexp::list([Sexp::atom(DATA)]));
    }

    #[test]
    fn eq_respects_element_order() {
        assert_ne!(
            Sexp::list([Sexp::atom("a"), Sexp::atom("b")]),
            Sexp::list([Sexp::atom("b"), Sexp::atom("a")])
        );
    }

    #[test]
    fn eq_respects_display_hints() {
        let hinted = Sexp::from(Atom::new(DATA).with_hint(HINT));

        assert_ne!(hinted, Sexp::atom(DATA));
    }

    #[test]
    fn eq_distinguishes_nesting() {
        assert_ne!(nested(1), nested(2));
    }

    #[test]
    fn hash_agrees_with_eq() {
        for sexp in [Sexp::atom(DATA), Sexp::list([]), nested(3)] {
            assert_eq!(hash_of(&sexp), hash_of(&sexp.clone()));
        }
    }

    #[test]
    fn clone_equals_the_original() {
        for sexp in [
            Sexp::atom(OCTETS),
            Sexp::from(Atom::new(DATA).with_hint(OCTETS)),
            Sexp::list([]),
            nested(5),
        ] {
            assert_eq!(sexp.clone(), sexp);
        }
    }

    // Debug

    #[test]
    fn debug_renders_an_atom_as_the_atom_does() {
        assert_eq!(format!("{:?}", Sexp::atom(DATA)), "4:data");
    }

    #[test]
    fn debug_renders_a_list_in_parentheses() {
        let sexp = Sexp::list([Sexp::atom("a"), Sexp::atom("bc")]);

        assert_eq!(format!("{sexp:?}"), "(1:a2:bc)");
    }

    #[test]
    fn debug_renders_the_empty_list() {
        assert_eq!(format!("{:?}", Sexp::list([])), "()");
    }

    #[test]
    fn debug_renders_nesting_and_hints() {
        let sexp = Sexp::list([
            Sexp::from(Atom::new(DATA).with_hint(HINT)),
            Sexp::list([Sexp::list([])]),
        ]);

        assert_eq!(format!("{sexp:?}"), "([4:hint]4:data(()))");
    }

    #[test]
    fn debug_escapes_non_printable_octets() {
        let sexp = Sexp::list([Sexp::atom([0xff, b'\n'])]);

        assert_eq!(format!("{sexp:?}"), r"(2:\xff\n)");
    }

    #[test]
    fn debug_does_not_recurse() {
        let deep = nested(DEEP);

        assert_eq!(format!("{deep:?}").len(), DEEP + "4:data".len() + DEEP);

        dismantle(deep);
    }

    // From, FromIterator, Extend

    #[test]
    fn from_octet_string_conversions_agree() {
        let expected = Sexp::atom(DATA);

        assert_eq!(Sexp::from("data"), expected);
        assert_eq!(Sexp::from(String::from("data")), expected);
        assert_eq!(Sexp::from(DATA), expected);
        assert_eq!(Sexp::from(DATA.to_vec()), expected);
        assert_eq!(Sexp::from(Box::from(DATA)), expected);
        assert_eq!(Sexp::from(Atom::new(DATA)), expected);
    }

    #[test]
    fn from_atom_keeps_the_display_hint() {
        let atom = Atom::new(DATA).with_hint(HINT);

        assert_eq!(Sexp::from(atom.clone()).into_atom(), Some(atom));
    }

    #[test]
    fn from_list_conversions_agree() {
        let expected = Sexp::list([Sexp::atom("a")]);

        assert_eq!(Sexp::from(vec![Sexp::atom("a")]), expected);
        assert_eq!(Sexp::from([Sexp::atom("a")]), expected);
    }

    #[test]
    fn from_an_empty_array_builds_the_empty_list() {
        assert_eq!(Sexp::from([]), Sexp::list([]));
    }

    #[test]
    fn from_iterator_collects_a_list() {
        let sexp: Sexp = ["a", "b"].into_iter().map(Sexp::from).collect();

        assert_eq!(sexp, Sexp::list([Sexp::atom("a"), Sexp::atom("b")]));
    }

    #[test]
    fn extend_appends_to_a_list() {
        let mut sexp = Sexp::list([Sexp::atom("a")]);

        sexp.extend([Sexp::atom("b")]);

        assert_eq!(sexp, Sexp::list([Sexp::atom("a"), Sexp::atom("b")]));
    }

    #[test]
    fn extend_replaces_an_atom_with_a_list() {
        let mut sexp = Sexp::atom(DATA);

        sexp.extend([Sexp::atom("b")]);

        assert_eq!(sexp, Sexp::list([Sexp::atom("b")]));
    }

    // sexp!

    #[test]
    fn macro_builds_a_list_of_atoms() {
        assert_eq!(
            sexp!["a", "b"],
            Sexp::list([Sexp::atom("a"), Sexp::atom("b")])
        );
    }

    #[test]
    fn macro_builds_the_empty_list() {
        assert_eq!(sexp![], Sexp::list([]));
    }

    #[test]
    fn macro_nests() {
        assert_eq!(
            sexp!["a", sexp!["b"]],
            Sexp::list([Sexp::atom("a"), Sexp::list([Sexp::atom("b")])])
        );
    }

    #[test]
    fn macro_accepts_a_hinted_atom() {
        let atom = Atom::new(DATA).with_hint(HINT);

        assert_eq!(sexp![atom.clone()], Sexp::list([Sexp::from(atom)]));
    }

    #[test]
    fn macro_accepts_a_trailing_comma() {
        assert_eq!(sexp!["a",], sexp!["a"]);
    }
}
