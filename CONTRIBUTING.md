# Contributing

Thank you for looking. Issues and pull requests are both welcome.

## Before a pull request

Four things run in CI, and running them first saves a round trip.

```sh
cargo test --all-targets
cargo test --doc
cargo clippy --all-targets
cargo +nightly fmt
```

Formatting needs nightly. `rustfmt.toml` sets the width and wrapping of
comments, which stable rustfmt reads and then ignores, so formatting on stable
leaves comments as they are and CI then disagrees with you.

## What the code is meant to be

This crate implements [RFC 9804]. Where the specification says something, the
code follows it and the documentation says which section it followed, in the
form of a section number in the prose. A pull request that departs from the
specification should say why in the same place.

A few rules hold throughout.

- The library has no dependencies, and no `unsafe`, which
  `#![forbid(unsafe_code)]` keeps true. A binding may depend on what it needs.
- Nothing panics on input from outside. A parse returns an `Error` saying what
  was wrong and where.
- Nothing recurses over a value whose depth the input controls. Parsing and
  writing both walk an explicit stack. Where recursion is unavoidable, as in
  dropping a tree, the depth is bounded by what the parser accepted.
- Every public item is documented, which `#![warn(missing_docs)]` keeps true.

## Tests

Tests live in a `#[cfg(test)] mod tests` at the end of the file they test, next
to the code rather than apart from it. They are named for the behaviour they
pin down rather than the function they call, and they are grouped under a
comment naming the item.

A change to how something behaves belongs with a test that would have failed
before it.

Two longer checks run on a schedule rather than on every push, and are worth
running by hand against a change to the parser.

```sh
cargo mutants
cargo +nightly fuzz run parse
```

A mutant that survives is a place where the code could be wrong and no test
would say so. What cargo-mutants reads is `.cargo/mutants.toml`, which leaves
out the handful of mutants that cannot change behaviour and says of each why
it cannot, so that anything a run reports is worth reading. A mutant that only
stops a loop making progress is caught by the suite never finishing, and is
reported apart from the rest.

Fuzzing wants Linux or macOS. cargo-fuzz links against libFuzzer, and the MSVC
linker has nothing to satisfy it with, so on Windows it fails to link whether
or not a sanitizer is asked for. The scheduled run covers it either way, and
WSL covers it by hand.

## Benchmarks

```sh
cargo bench
```

The shapes measured are in `benches/common`, and each stands for something
that turns up in practice rather than something that flatters a number. A
certificate of the kind SPKI puts in one, octet strings that carry display
hints, a wide list, one large octet string, and lists nested as deeply as a
parser accepts.

The harness counts allocations as well as time, which is usually the number
that matters here, since reading and writing are mostly a question of how much
gets copied. Measure before changing anything for speed, and say in the pull
request what the measurement was.

## The Python bindings

The library is the whole of what the bindings carry, so a change to behaviour
belongs in `src` and the binding follows it. `bindings/python` holds only the
conversions between a Python value and a `Sexp`, and the classes and functions
that carry them across.

They are built and tested with their own tools, from `bindings/python`.

```sh
uv venv
uv pip install --group dev
uv run maturin develop
uv run pytest
uv run ruff format .
uv run ruff check .
uv run mypy .
```

The same standard holds on that side of the boundary. Everything public is
documented and annotated, `ruff` formats and lints it, and `mypy` checks it
under `strict`. The type stubs in `python/csexpr/_csexpr.pyi` are written by
hand, so a change to a signature in Rust belongs there in the same commit.

## Commits

One concept per commit, and a subject line of the form `type: what changed`,
lowercase, with no trailing full stop. No body is needed for a change that the
subject already describes.

## Releasing

1. Move the entries under `Unreleased` in `CHANGELOG.md` to a new version.
2. Raise `version` under `[workspace.package]` in `Cargo.toml`, which is the
   one place it is written, and commit.
3. Tag it `vX.Y.Z` and push the tag.

Pushing the tag publishes the crate to crates.io and the wheels to PyPI, both
through Trusted Publishing, so neither registry needs a token stored here.
Nothing is published unless the tag is green, since the release runs the whole
of CI against the tagged commit before it publishes anything. It also refuses a
tag that disagrees with the manifest or names a version the changelog does not
mention.

[RELEASING.md](RELEASING.md) has the rest, including what has to be set up at
each registry before the first release and what to do when a release fails
halfway.

[RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html
