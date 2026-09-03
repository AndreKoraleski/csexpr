"""Writing the three representations of RFC 9804 §6."""

import csexpr
import pytest
from csexpr import Atom

CORPUS: list[csexpr.Writable] = [
    b"",
    b"data",
    b"a b",
    bytes([0x00, 0xFF, 0x1B, 0x7F]),
    b"x" * 300,
    Atom(b"bob", hint=b"text/plain"),
    Atom(b"", hint=b""),
    Atom(bytes([0xFF]), hint=bytes([0x00])),
    [],
    [b"issuer", b"bob"],
    [[], [b"a", [b"b", b"c"]]],
]


def test_writes_the_canonical_representation() -> None:
    assert csexpr.to_canonical(b"data") == b"4:data"
    assert csexpr.to_canonical(b"") == b"0:"
    assert csexpr.to_canonical([]) == b"()"
    assert csexpr.to_canonical([b"issuer", b"bob"]) == b"(6:issuer3:bob)"


def test_writes_a_display_hint() -> None:
    hinted = Atom(b"bob", hint=b"text/plain")

    assert csexpr.to_canonical(hinted) == b"[10:text/plain]3:bob"


def test_writes_the_basic_transport_representation() -> None:
    assert csexpr.to_transport([b"issuer", b"bob"]) == "{KDY6aXNzdWVyMzpib2Ip}"
    assert csexpr.to_transport([]) == "{KCk=}"


def test_writes_the_advanced_representation() -> None:
    assert csexpr.to_advanced([b"issuer", b"bob"]) == "(issuer bob)"
    assert csexpr.to_advanced(b"hello world") == '"hello world"'
    assert csexpr.to_advanced(bytes([0xFF, 0x00])) == "#ff00#"
    assert csexpr.to_advanced([]) == "()"


def test_takes_text_as_utf_8() -> None:
    assert csexpr.to_canonical("data") == b"4:data"
    assert csexpr.to_canonical(["issuer", "bob"]) == b"(6:issuer3:bob)"
    assert csexpr.to_canonical("héllo") == b"6:h\xc3\xa9llo"


def test_takes_a_tuple_as_a_list() -> None:
    assert csexpr.to_canonical((b"a", b"b")) == b"(1:a1:b)"
    assert csexpr.to_canonical((b"a", (b"b",))) == b"(1:a(1:b))"


def test_takes_octets_in_any_form_python_lends_them() -> None:
    assert csexpr.to_canonical(bytearray(b"data")) == b"4:data"
    assert csexpr.to_canonical(memoryview(b"data")) == b"4:data"


def test_distinguishes_the_empty_list_from_the_empty_octet_string() -> None:
    assert csexpr.to_canonical([]) != csexpr.to_canonical(b"")


@pytest.mark.parametrize("sexp", CORPUS)
def test_reads_back_what_it_wrote(sexp: csexpr.Writable) -> None:
    assert csexpr.parse(csexpr.to_canonical(sexp)) == sexp
    assert csexpr.parse(csexpr.to_transport(sexp)) == sexp
    assert csexpr.parse(csexpr.to_advanced(sexp)) == sexp


@pytest.mark.parametrize("sexp", CORPUS)
def test_the_canonical_representation_survives_a_round_trip_octet_for_octet(
    sexp: csexpr.Writable,
) -> None:
    written = csexpr.to_canonical(sexp)

    assert csexpr.to_canonical(csexpr.parse_canonical(written)) == written


@pytest.mark.parametrize("value", [3, None, {b"a": b"b"}, {b"a"}, object()])
def test_refuses_what_is_not_an_s_expression(value: object) -> None:
    with pytest.raises(TypeError):
        csexpr.to_canonical(value)  # type: ignore[arg-type]


def test_refuses_what_is_not_an_s_expression_within_a_list() -> None:
    with pytest.raises(TypeError):
        csexpr.to_canonical([b"a", 3])  # type: ignore[list-item]


def test_writes_deeply_nested_lists_without_crashing() -> None:
    depth = csexpr.DEFAULT_MAX_DEPTH
    sexp: csexpr.Writable = []

    for _ in range(depth - 1):
        sexp = [sexp]

    assert csexpr.to_canonical(sexp) == b"(" * depth + b")" * depth


def test_refuses_lists_nested_deeper_than_it_can_write() -> None:
    sexp: csexpr.Writable = []

    for _ in range(csexpr.DEFAULT_MAX_DEPTH + 10):
        sexp = [sexp]

    with pytest.raises(ValueError, match="nested deeper"):
        csexpr.to_canonical(sexp)
