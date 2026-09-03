"""An octet string carrying a display hint (§4.6)."""

import pytest
from csexpr import DEFAULT_HINT, Atom


def test_holds_the_data_octets() -> None:
    assert Atom(b"data").data == b"data"


def test_holds_no_hint_unless_given_one() -> None:
    assert Atom(b"data").hint is None
    assert Atom(b"data", hint=b"hint").hint == b"hint"


def test_takes_text_as_utf_8() -> None:
    assert Atom("data").data == b"data"
    assert Atom("héllo").data == "héllo".encode()
    assert Atom("data", hint="text/plain").hint == b"text/plain"


def test_takes_octets_in_any_form_python_lends_them() -> None:
    assert Atom(bytearray(b"data")).data == b"data"
    assert Atom(memoryview(b"data")).data == b"data"


def test_takes_zero_length_data_and_hints() -> None:
    assert Atom(b"").data == b""
    assert Atom(b"data", hint=b"").hint == b""


def test_takes_arbitrary_octets() -> None:
    octets = bytes([0x00, 0xFF, 0x1B, 0x7F])

    assert Atom(octets).data == octets
    assert Atom(b"data", hint=octets).hint == octets


def test_refuses_what_is_not_octets() -> None:
    with pytest.raises(TypeError):
        Atom(3)  # type: ignore[arg-type]

    with pytest.raises(TypeError):
        Atom(b"data", hint=[b"hint"])  # type: ignore[arg-type]


def test_equality_compares_data_and_hint() -> None:
    assert Atom(b"data", hint=b"hint") == Atom(b"data", hint=b"hint")
    assert Atom(b"data", hint=b"hint") != Atom(b"data", hint=b"other")
    assert Atom(b"data", hint=b"hint") != Atom(b"other", hint=b"hint")


def test_equality_distinguishes_an_absent_hint_from_a_present_one() -> None:
    assert Atom(b"data") != Atom(b"data", hint=b"hint")
    assert Atom(b"data") != Atom(b"data", hint=b"")
    assert Atom(b"data") != Atom(b"data", hint=DEFAULT_HINT)


def test_equality_is_case_sensitive() -> None:
    assert Atom(b"data") != Atom(b"DATA")
    assert Atom(b"d", hint=b"h") != Atom(b"d", hint=b"H")


def test_an_atom_without_a_hint_equals_the_octets_it_holds() -> None:
    # Plain bytes is what an octet string without a hint is, so the two stand
    # for the same S-expression.
    assert Atom(b"data") == b"data"
    assert Atom(b"data") == b"data"
    assert Atom(b"data", hint=b"hint") != b"data"


def test_equality_against_something_else_entirely_is_false() -> None:
    assert Atom(b"data") != 3
    assert Atom(b"data") != [b"data"]
    assert Atom(b"data") is not None


def test_ordering_is_not_defined() -> None:
    # RFC 9804 defines no ordering for octet strings.
    with pytest.raises(TypeError):
        _ = Atom(b"a") < Atom(b"b")  # type: ignore[operator]


def test_hash_agrees_with_equality() -> None:
    assert hash(Atom(b"d", hint=b"h")) == hash(Atom(b"d", hint=b"h"))
    assert len({Atom(b"d", hint=b"h"), Atom(b"d", hint=b"h")}) == 1
    assert len({Atom(b"d"), Atom(b"d", hint=b"h")}) == 2


def test_length_counts_the_data_and_not_the_hint() -> None:
    assert len(Atom(b"data")) == 4
    assert len(Atom(b"data", hint=b"a longer hint")) == 4
    assert len(Atom(b"")) == 0


def test_effective_hint_prefers_the_hint_it_has() -> None:
    assert Atom(b"d", hint=b"hint").effective_hint(DEFAULT_HINT) == b"hint"
    assert Atom(b"d").effective_hint(DEFAULT_HINT) == DEFAULT_HINT
    assert Atom(b"d", hint=b"").effective_hint(b"other") == b""


def test_eq_ignoring_hint_compares_only_the_data() -> None:
    assert Atom(b"d", hint=b"hint").eq_ignoring_hint(Atom(b"d", hint=b"other"))
    assert Atom(b"d", hint=b"hint").eq_ignoring_hint(b"d")
    assert not Atom(b"d", hint=b"hint").eq_ignoring_hint(b"other")


def test_repr_says_what_it_holds() -> None:
    assert repr(Atom(b"data")) == 'Atom(b"data")'
    assert repr(Atom(b"d", hint=b"h")) == 'Atom(b"d", hint=b"h")'
    assert repr(Atom(bytes([0xFF]))) == 'Atom(b"\\xff")'


def test_the_default_hint_is_the_media_type_the_rfc_names() -> None:
    assert DEFAULT_HINT == b"application/octet-stream"
