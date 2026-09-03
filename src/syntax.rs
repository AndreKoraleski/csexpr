//! The character rules the representations are written and read by.
//!
//! RFC 9804 §3 fixes the character set, and §4.3 fixes which octet strings may
//! stand as a token. Both the writer of the advanced representation and the
//! reader depend on exactly these rules, so they are stated once here.

/// The eight punctuation marks §4.3 calls pseudo-alphabetic, which may appear
/// in a token alongside letters and digits.
const PUNCTUATION: &[u8; 8] = b"-./_:*+=";

/// Returns `true` if the octet may appear in a token (§4.3).
pub(crate) fn is_token(octet: u8) -> bool {
    octet.is_ascii_alphanumeric() || PUNCTUATION.contains(&octet)
}

/// Returns `true` if the octet may begin a token (§4.3).
///
/// A token may not begin with a digit, since a leading digit begins the length
/// that §4.1, §4.2, §4.4 and §4.5 allow before an octet string.
pub(crate) fn is_token_start(octet: u8) -> bool {
    octet.is_ascii_alphabetic() || PUNCTUATION.contains(&octet)
}

/// Returns `true` if the octet string may be written as a token (§4.3).
///
/// §4.3 asks for three things of a token. It is one octet or longer, it does
/// not begin with a digit, and every octet in it is a letter, a digit, or one
/// of the eight punctuation marks.
pub(crate) fn qualifies_as_token(octets: &[u8]) -> bool {
    match octets {
        [] => false,
        [first, rest @ ..] => is_token_start(*first) && rest.iter().copied().all(is_token),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every octet, so that a rule about all of them is tested on all of them.
    fn all_octets() -> impl Iterator<Item = u8> {
        0..=u8::MAX
    }

    // is_token, is_token_start

    #[test]
    fn is_token_admits_letters_digits_and_the_eight_marks() {
        for octet in all_octets() {
            let expected = octet.is_ascii_alphanumeric() || b"-./_:*+=".contains(&octet);

            assert_eq!(is_token(octet), expected, "octet {octet:#04x}");
        }
    }

    #[test]
    fn is_token_start_admits_what_is_token_admits_but_no_digit() {
        for octet in all_octets() {
            let expected = is_token(octet) && !octet.is_ascii_digit();

            assert_eq!(is_token_start(octet), expected, "octet {octet:#04x}");
        }
    }

    #[test]
    fn is_token_rejects_the_syntax_characters() {
        for octet in *b"()[]{}|#\"\\ \t\n" {
            assert!(!is_token(octet), "octet {octet:#04x}");
        }
    }

    #[test]
    fn is_token_rejects_octets_outside_ascii() {
        for octet in 0x80..=u8::MAX {
            assert!(!is_token(octet), "octet {octet:#04x}");
        }
    }

    // qualifies_as_token

    #[test]
    fn qualifies_as_token_accepts_letters_digits_and_marks() {
        assert!(qualifies_as_token(b"issuer"));
        assert!(qualifies_as_token(b"text/plain"));
        assert!(qualifies_as_token(b"a1"));
        assert!(qualifies_as_token(b"-.foo_:*+="));
    }

    #[test]
    fn qualifies_as_token_accepts_one_octet() {
        assert!(qualifies_as_token(b"a"));
        assert!(qualifies_as_token(b"="));
    }

    #[test]
    fn qualifies_as_token_rejects_nothing_at_all() {
        assert!(!qualifies_as_token(b""));
    }

    #[test]
    fn qualifies_as_token_rejects_a_leading_digit() {
        assert!(!qualifies_as_token(b"1"));
        assert!(!qualifies_as_token(b"3abc"));
    }

    #[test]
    fn qualifies_as_token_rejects_anything_else_anywhere() {
        assert!(!qualifies_as_token(b"a b"));
        assert!(!qualifies_as_token(b"a(b"));
        assert!(!qualifies_as_token(b"a\"b"));
        assert!(!qualifies_as_token(&[b'a', 0x00]));
        assert!(!qualifies_as_token(&[b'a', 0xff]));
    }
}
