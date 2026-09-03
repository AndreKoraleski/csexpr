# csexpr

S-expressions, as specified by [RFC 9804]. The name is short for canonical
S-expression, which is what this format has long been called after the one
representation of it that is unique, and by which it is known where it is used,
as in SPKI and SDSI.

This package is the Rust crate of the same name, bound to Python. The parsing
and the writing happen in Rust, and no Python code stands between them and you.

## Install

```sh
pip install csexpr
```

## Use

An S-expression is either an octet string or a list of simpler S-expressions,
and a Python value stands for one directly. `bytes` is an octet string, a
`list` is a list, and `Atom` is an octet string that carries a display hint.
Text is accepted wherever octets are, and is taken as UTF-8.

```python
import csexpr

assert csexpr.to_canonical(["issuer", "bob"]) == b"(6:issuer3:bob)"
assert csexpr.parse(b"(6:issuer3:bob)") == [b"issuer", b"bob"]

# The three representations of §6 all carry the same value.
assert csexpr.parse(b"(issuer bob)") == [b"issuer", b"bob"]
assert csexpr.parse(b"{KDY6aXNzdWVyMzpib2Ip}") == [b"issuer", b"bob"]

# A display hint says how an octet string should be shown, and nothing else.
bob = csexpr.Atom(b"bob", hint=b"text/plain")

assert csexpr.to_advanced(["issuer", bob]) == "(issuer [text/plain]bob)"
assert csexpr.parse(b"(issuer [text/plain]bob)") == ["issuer".encode(), bob]
```

## The three representations

| Written by | Looks like | For |
| --- | --- | --- |
| `to_canonical` (§6.2) | `(6:issuer3:bob)` | Hashing and signing, since one S-expression has exactly one of these |
| `to_transport` (§6.3) | `{KDY6aXNzdWVyMzpib2Ip}` | Channels that would disturb raw octets |
| `to_advanced` (§6.4) | `(issuer bob)` | Being read by a person |

`parse` reads whichever of the three it is given. `parse_canonical` reads only
the first, which is what verifying a signature calls for, since a signature is
computed over those octets exactly.

## Reading what cannot be trusted

`Parser` bounds the size of the input and of every octet string in it, and
turns off whichever constructs an application has no use for. Each of the
restrictions RFC 9804 §8 names is one keyword.

```python
from csexpr import ParseError, Parser

parser = Parser(
    canonical=True,
    max_input_len=64 * 1024,
    max_atom_len=4096,
    max_depth=32,
    allow_display_hints=False,
    allow_empty_lists=False,
)

assert parser.parse(b"(6:issuer3:bob)") == [b"issuer", b"bob"]

try:
    parser.parse(b"(issuer bob)")
except ParseError as error:
    print(error.kind, error.offset)
```

A `ParseError` carries `offset`, the octet the offending construct begins at,
and `kind`, a short name for what was wrong.

Parsing takes no stack in proportion to how deeply the input nests, and no
memory on the strength of a length the input states. Lists may nest as deeply
as `DEFAULT_MAX_DEPTH`, and writing refuses a structure nested deeper than
that rather than risking the interpreter.

## License

MIT, see [LICENSE](LICENSE).

[RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html
