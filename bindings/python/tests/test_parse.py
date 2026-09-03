"""Reading the three representations of RFC 9804 §6."""

import csexpr
import pytest
from csexpr import Atom, ParseError, Parser


def test_reads_a_verbatim_octet_string() -> None:
    assert csexpr.parse(b"4:data") == b"data"
    assert csexpr.parse(b"0:") == b""


def test_reads_a_list() -> None:
    assert csexpr.parse(b"(6:issuer3:bob)") == [b"issuer", b"bob"]
    assert csexpr.parse(b"()") == []
    assert csexpr.parse(b"(()(1:a))") == [[], [b"a"]]


def test_reads_a_display_hint() -> None:
    assert csexpr.parse(b"[4:hint]4:data") == Atom(b"data", hint=b"hint")


def test_reads_the_advanced_representation() -> None:
    assert csexpr.parse(b"(issuer bob)") == [b"issuer", b"bob"]
    assert csexpr.parse(b'"hello world"') == b"hello world"
    assert csexpr.parse(b"#616263#") == b"abc"
    assert csexpr.parse(b"|YWJj|") == b"abc"
    assert csexpr.parse(b"  (a b)  ") == [b"a", b"b"]


def test_reads_the_basic_transport_representation() -> None:
    assert csexpr.parse(b"{KDY6aXNzdWVyMzpib2Ip}") == [b"issuer", b"bob"]


def test_reads_the_three_representations_as_one_value() -> None:
    canonical = csexpr.parse(b"(6:issuer3:bob)")
    advanced = csexpr.parse(b"(issuer bob)")
    transport = csexpr.parse(b"{KDY6aXNzdWVyMzpib2Ip}")

    assert canonical == advanced == transport


def test_reads_octets_given_in_any_form() -> None:
    assert csexpr.parse(bytearray(b"4:data")) == b"data"
    assert csexpr.parse("4:data") == b"data"


def test_distinguishes_the_empty_list_from_the_empty_octet_string() -> None:
    assert csexpr.parse(b"()") == []
    assert csexpr.parse(b"0:") == b""
    assert csexpr.parse(b"()") != csexpr.parse(b"0:")


def test_parse_canonical_reads_only_the_canonical_representation() -> None:
    assert csexpr.parse_canonical(b"(6:issuer3:bob)") == [b"issuer", b"bob"]

    with pytest.raises(ParseError):
        csexpr.parse_canonical(b"(issuer bob)")


def test_a_failure_says_what_was_wrong_and_where() -> None:
    with pytest.raises(ParseError) as raised:
        csexpr.parse(b"(1:a 01:b)")

    assert raised.value.kind == "length_leading_zero"
    assert raised.value.offset == 5
    assert "offset 5" in str(raised.value)


def test_a_failure_is_a_value_error() -> None:
    with pytest.raises(ValueError, match="ended in the middle"):
        csexpr.parse(b"(a")


@pytest.mark.parametrize(
    ("data", "kind"),
    [
        (b"", "unexpected_end"),
        (b"(a", "unexpected_end"),
        (b")", "unmatched_parenthesis"),
        (b"4:data4:more", "trailing_octets"),
        (b"04:data", "length_leading_zero"),
        (b'2"abc"', "length_mismatch"),
        (b"#616#", "odd_hex_digits"),
        (b"#6g#", "invalid_hex_digit"),
        (b"|YW*j|", "invalid_base64"),
        (b'"\\q"', "invalid_escape"),
        (b"[4:hint](1:a)", "hint_on_list"),
    ],
)
def test_each_kind_of_failure_is_named(data: bytes, kind: str) -> None:
    with pytest.raises(ParseError) as raised:
        csexpr.parse(data)

    assert raised.value.kind == kind


def test_a_parser_reads_what_it_is_built_to_read() -> None:
    parser = Parser()

    assert parser.parse(b"(6:issuer3:bob)") == [b"issuer", b"bob"]
    assert parser.parse(b"(issuer bob)") == [b"issuer", b"bob"]


def test_a_canonical_parser_refuses_the_other_representations() -> None:
    parser = Parser(canonical=True)

    assert parser.parse(b"(1:a)") == [b"a"]

    for data in [b"(a)", b"{KCk=}", b" 1:a"]:
        with pytest.raises(ParseError):
            parser.parse(data)


def test_max_depth_bounds_how_deeply_lists_nest() -> None:
    parser = Parser(max_depth=2)

    assert parser.parse(b"((1:a))") == [[b"a"]]

    with pytest.raises(ParseError) as raised:
        parser.parse(b"(((1:a)))")

    assert raised.value.kind == "too_deep"


def test_max_atom_len_bounds_an_octet_string() -> None:
    parser = Parser(max_atom_len=3)

    assert parser.parse(b"3:abc") == b"abc"

    with pytest.raises(ParseError) as raised:
        parser.parse(b"4:abcd")

    assert raised.value.kind == "atom_too_long"


def test_max_input_len_bounds_the_input() -> None:
    parser = Parser(max_input_len=6)

    assert parser.parse(b"4:data") == b"data"

    with pytest.raises(ParseError) as raised:
        parser.parse(b"4:datax")

    assert raised.value.kind == "input_too_long"


@pytest.mark.parametrize(
    ("keyword", "refused", "accepted"),
    [
        ("allow_display_hints", b"[1:h]1:a", b"1:a"),
        ("allow_empty_atoms", b"0:", b"1:a"),
        ("allow_empty_lists", b"()", b"(1:a)"),
        ("allow_list_as_first_element", b"((1:a)1:b)", b"(1:b(1:a))"),
        ("allow_hexadecimal", b"#616263#", b"|YWJj|"),
        ("allow_base64", b"|YWJj|", b"#616263#"),
        ("allow_lengths", b'3"abc"', b"3:abc"),
        ("allow_transport", b"{KCk=}", b"()"),
        ("allow_advanced", b"(a b)", b"(1:a1:b)"),
    ],
)
def test_each_restriction_of_section_eight_can_be_turned_on(
    keyword: str, refused: bytes, accepted: bytes
) -> None:
    parser = Parser(**{keyword: False})

    with pytest.raises(ParseError):
        parser.parse(refused)

    assert parser.parse(accepted) is not None


def test_restrictions_reach_inside_the_braces() -> None:
    parser = Parser(allow_empty_lists=False)

    with pytest.raises(ParseError) as raised:
        parser.parse(b"{KCk=}")

    assert raised.value.kind == "empty_list_not_allowed"


def test_parse_prefix_says_how_far_it_read() -> None:
    parser = Parser()

    assert parser.parse_prefix(b"4:data4:more") == (b"data", 6)
    assert parser.parse_prefix(b"(1:a)(1:b)") == ([b"a"], 5)


def test_parse_prefix_reads_a_stream_one_at_a_time() -> None:
    parser = Parser()
    data = b"1:a1:b1:c"
    offset = 0
    read = []

    while offset < len(data):
        sexp, length = parser.parse_prefix(data[offset:])
        read.append(sexp)
        offset += length

    assert read == [b"a", b"b", b"c"]


def test_a_parser_may_be_used_more_than_once() -> None:
    parser = Parser()

    assert parser.parse(b"1:a") == b"a"
    assert parser.parse(b"1:b") == b"b"


def test_reading_deeply_nested_input_does_not_crash() -> None:
    depth = csexpr.DEFAULT_MAX_DEPTH
    data = b"(" * depth + b")" * depth
    sexp = csexpr.parse(data)

    for _ in range(depth - 1):
        assert isinstance(sexp, list)
        sexp = sexp[0]

    assert sexp == []
