# Changelog

Everything worth telling a user of this crate about, newest first. The format
follows [Keep a Changelog], and the versions follow [Semantic Versioning].

Raising the minimum supported Rust version counts as a change worth a minor
release, and is noted here whenever it happens.

## [Unreleased]

### Added

- Python bindings, published to PyPI as `csexpr`. A Python value stands for an
  S-expression directly, so `bytes` is an octet string, a `list` is a list,
  and `Atom` is an octet string that carries a display hint. Everything the
  Rust library reads and writes is reachable, restrictions included, and the
  package is typed.

## [0.1.0]

First release.

### Added

- `Atom`, an octet string with the optional display hint of RFC 9804 §4.6, and
  `Sexp`, an S-expression, with traversal by `get`, `depth` and `preorder` and
  building by `From`, `FromIterator`, `Extend` and the `sexp!` macro.
- `encode::canonical`, `encode::transport` and `encode::advanced`, writing the
  three representations of §6 to a `Vec`, a `String` or an `io::Write`.
- `decode::parse`, reading all three, and `decode::parse_canonical`, reading
  only the representation a signature is computed over.
- `decode::Parser`, carrying every restriction §8 names, along with limits on
  the depth of nesting, the length of an octet string and the size of the
  input.
- `decode::Error`, saying what was wrong and at which octet.

[Keep a Changelog]: https://keepachangelog.com/en/1.1.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html
[Unreleased]: https://github.com/AndreKoraleski/csexpr/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/AndreKoraleski/csexpr/releases/tag/v0.1.0
