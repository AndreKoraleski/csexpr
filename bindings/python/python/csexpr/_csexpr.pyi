"""Types for the extension module, which the package re-exports."""

from collections.abc import Sequence
from typing import TypeAlias

Sexp: TypeAlias = bytes | Atom | list[Sexp]
"""What reading an S-expression yields.

``bytes`` is an octet string, a ``list`` is a list, and :class:`Atom` is an
octet string that carries a display hint.
"""

Octets: TypeAlias = bytes | bytearray | memoryview | str | Atom
"""What may be given wherever octets are wanted. Text is taken as UTF-8."""

Writable: TypeAlias = Octets | Sequence[Writable]
"""What may be written, which is more than reading gives back.

A sequence here means a ``list`` or a ``tuple``. It is spelled as the wider
type because a ``list`` is invariant, and a ``list`` of what reading gives
back would otherwise not be writable.
"""

DEFAULT_HINT: bytes
"""The display hint §4.6 names for an application that specifies no other."""

DEFAULT_MAX_DEPTH: int
"""How deeply lists may nest before a parser given no other limit refuses."""

class ParseError(ValueError):
    """Raised where input is not an S-expression the parser accepts."""

    offset: int
    """The octet the offending construct begins at."""

    kind: str
    """A short name for what was wrong, such as ``"unexpected_end"``."""

class Atom:
    """An octet string with a display hint (§4.6).

    Building one is the only way to give an octet string a hint, since plain
    ``bytes`` carries none.
    """

    def __init__(self, data: Octets, hint: Octets | None = None) -> None: ...
    @property
    def data(self) -> bytes:
        """The data octets."""

    @property
    def hint(self) -> bytes | None:
        """The display hint, or ``None`` where the atom carries none."""

    def effective_hint(self, default: Octets) -> bytes:
        """Return the display hint, or ``default`` where there is none."""

    def eq_ignoring_hint(self, other: Octets) -> bool:
        """Return whether the data octets are equal, whatever the hints."""

    def __len__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class Parser:
    """A reader of S-expressions, and the restrictions it reads against.

    Built with no arguments it accepts every representation of §6. Each
    keyword turns on one of the restrictions §8 lists, or bounds how large or
    how deeply nested the input may be.
    """

    def __init__(
        self,
        *,
        canonical: bool = False,
        max_depth: int = ...,
        max_atom_len: int | None = None,
        max_input_len: int | None = None,
        allow_advanced: bool = True,
        allow_transport: bool = True,
        allow_display_hints: bool = True,
        allow_empty_atoms: bool = True,
        allow_empty_lists: bool = True,
        allow_list_as_first_element: bool = True,
        allow_hexadecimal: bool = True,
        allow_base64: bool = True,
        allow_lengths: bool = True,
    ) -> None: ...
    def parse(self, data: Octets) -> Sexp:
        """Read one S-expression, which is the whole of the input."""

    def parse_prefix(self, data: Octets) -> tuple[Sexp, int]:
        """Read one S-expression, and say how many octets it occupied."""

def parse(data: Octets) -> Sexp:
    """Read one S-expression, in whichever representation of §6 it is in."""

def parse_canonical(data: Octets) -> Sexp:
    """Read one S-expression in the canonical representation of §6.2."""

def to_canonical(sexp: Writable) -> bytes:
    """Return the canonical representation of the S-expression (§6.2)."""

def to_transport(sexp: Writable) -> str:
    """Return the basic transport representation of the S-expression (§6.3)."""

def to_advanced(sexp: Writable) -> str:
    """Return the advanced representation of the S-expression (§6.4)."""
