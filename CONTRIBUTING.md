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

## Commits

One concept per commit, and a subject line of the form `type: what changed`,
lowercase, with no trailing full stop. No body is needed for a change that the
subject already describes.

## Releasing

1. Move the entries under `Unreleased` in `CHANGELOG.md` to a new version.
2. Set the version in `Cargo.toml`, and commit.
3. Tag it `vX.Y.Z` and push the tag.

Pushing the tag publishes the crate. The workflow refuses a tag that disagrees
with the manifest or names a version the changelog does not mention.

[RFC 9804]: https://www.rfc-editor.org/rfc/rfc9804.html
