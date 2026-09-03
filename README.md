# csexpr

[![CI](https://github.com/AndreKoraleski/csexpr/actions/workflows/ci.yml/badge.svg)](https://github.com/AndreKoraleski/csexpr/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/csexpr?logo=rust)](https://crates.io/crates/csexpr)
[![docs.rs](https://img.shields.io/docsrs/csexpr?logo=docsdotrs)](https://docs.rs/csexpr)
[![PyPI](https://img.shields.io/pypi/v/csexpr?logo=pypi&logoColor=white)](https://pypi.org/project/csexpr/)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

S-expressions, as specified by [RFC 9804]. The name is short for canonical
S-expression, which is what this format has long been called after the one
representation of it that is unique, and by which it is known where it is used,
as in SPKI and SDSI.

An S-expression is either an octet string or a list of simpler S-expressions.
An octet string may carry a display hint, which says how it should be shown to
a person and means nothing else. RFC 9804 gives one S-expression three ways of
being written, and this crate reads all three and writes all three.

## Install

```sh
cargo add csexpr
```

## Use

```rust
use csexpr::{Atom, decode, encode::canonical, sexp};

let cert = sexp!["issuer", Atom::new("bob").with_hint("text/plain")];
let written = canonical::to_vec(&cert);

assert_eq!(written, b"(6:issuer[10:text/plain]3:bob)");
assert_eq!(decode::parse(&written).unwrap(), cert);

// The advanced representation carries the same value.
assert_eq!(cert.to_string(), "(issuer [text/plain]bob)");
assert_eq!(decode::parse(b"(issuer [text/plain]bob)").unwrap(), cert);
```

## The three representations

| Representation | Written by | Looks like | For |
| --- | --- | --- | --- |
| Canonical (§6.2) | `encode::canonical` | `(6:issuer3:bob)` | Hashing and signing, since one S-expression has exactly one of these |
| Basic transport (§6.3) | `encode::transport` | `{KDY6aXNzdWVyMzpib2Ip}` | Channels that would disturb raw octets |
| Advanced (§6.4) | `encode::advanced` | `(issuer bob)` | Being read by a person |

`decode::parse` reads whichever of the three it is given.
`decode::parse_canonical` reads only the first, which is what verifying a
signature calls for, since a signature is computed over those octets exactly.

## Reading what cannot be trusted

Parsing takes no stack in proportion to how deeply the input nests, and no
memory on the strength of a length the input states. What a parse builds is a
tree, and dropping, cloning or comparing a tree does recurse, so a parser
refuses input nested deeper than `DEFAULT_MAX_DEPTH` unless it is told
otherwise.

`decode::Parser` bounds the size of the input and of every octet string in it,
and turns off whichever constructs an application has no use for. Each of the
restrictions RFC 9804 §8 names is one call.

```rust
use csexpr::decode::Parser;

let parser = Parser::canonical()
    .max_input_len(64 * 1024)
    .max_atom_len(4096)
    .max_depth(32)
    .allow_display_hints(false)
    .allow_empty_lists(false);

assert!(parser.parse(b"(6:issuer3:bob)").is_ok());
assert!(parser.parse(b"(issuer bob)").is_err());
```

## What this crate is

- The library itself has no dependencies. A binding to another language
  depends on what that language needs and nothing more.
- No `unsafe` code, which `#![forbid(unsafe_code)]` keeps true.
- Minimum supported Rust version 1.85, raised only in a minor release.

## Python

The same library, bound to Python, lives in [bindings/python](bindings/python).
The parsing and the writing happen in Rust either way.

```sh
pip install csexpr
```

```python
import csexpr

assert csexpr.to_canonical(["issuer", "bob"]) == b"(6:issuer3:bob)"
assert csexpr.parse(b"(6:issuer3:bob)") == [b"issuer", b"bob"]
```

A Python value stands for an S-expression directly. `bytes` is an octet
string, a `list` is a list, and `Atom` is an octet string that carries a
display hint. The rest is in [its own README](bindings/python/README.md).

The wheels are built against the stable ABI, so one per platform serves every
Python from 3.10 up.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). To report something that looks like a
security problem, see [SECURITY.md](SECURITY.md) instead of opening an issue.

## License

MIT, see [LICENSE](LICENSE).

[RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html
