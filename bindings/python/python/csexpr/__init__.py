"""S-expressions, as specified by RFC 9804.

The name is short for canonical S-expression, which is what this format has
long been called after the one representation of it that is unique, and by
which it is known where it is used, as in SPKI and SDSI.

An S-expression is either an octet string or a list of simpler S-expressions,
and a Python value stands for one directly. ``bytes`` is an octet string, a
``list`` is a list, and :class:`Atom` is an octet string that carries a display
hint. Text is accepted wherever octets are, and is taken as UTF-8, which §4.6
recommends where the data is text.

    >>> import csexpr
    >>> csexpr.to_canonical(["issuer", "bob"])
    b'(6:issuer3:bob)'
    >>> csexpr.parse(b"(6:issuer3:bob)")
    [b'issuer', b'bob']
    >>> csexpr.to_advanced([b"issuer", csexpr.Atom(b"bob", hint=b"text/plain")])
    '(issuer [text/plain]bob)'
"""

from collections.abc import Sequence
from typing import TypeAlias

from ._csexpr import (
    DEFAULT_HINT,
    DEFAULT_MAX_DEPTH,
    Atom,
    ParseError,
    Parser,
    parse,
    parse_canonical,
    to_advanced,
    to_canonical,
    to_transport,
)

Sexp: TypeAlias = bytes | Atom | list["Sexp"]
"""What reading an S-expression yields."""

Octets: TypeAlias = bytes | bytearray | memoryview | str | Atom
"""What may be given wherever octets are wanted."""

Writable: TypeAlias = Octets | Sequence["Writable"]
"""What may be written, which is more than reading gives back.

A sequence here means a ``list`` or a ``tuple``. It is spelled as the wider
type because a ``list`` is invariant, and a ``list`` of what reading gives
back would otherwise not be writable.
"""

__all__ = [
    "DEFAULT_HINT",
    "DEFAULT_MAX_DEPTH",
    "Atom",
    "Octets",
    "ParseError",
    "Parser",
    "Sexp",
    "Writable",
    "parse",
    "parse_canonical",
    "to_advanced",
    "to_canonical",
    "to_transport",
]
