# Releasing

Pushing a `vX.Y.Z` tag publishes the crate to crates.io and the wheels to
PyPI. Neither registry holds a token here, since both are reached through
Trusted Publishing, which hands the workflow a short-lived one against the
identity GitHub vouches for. That has to be set up once before it can work.

## Once, before the first release

### crates.io

crates.io has no pending publisher, so a crate has to exist before a trusted
publisher can be attached to it. Version 0.1.0 therefore goes up by hand, and
every version after it goes up by tag.

1. Make a token at <https://crates.io/settings/tokens>, scoped to
   `publish-new` and `publish-update`, with the shortest expiry that suits.
2. Run `cargo login` and give it the token.
3. From a clean checkout, run `cargo publish --dry-run` and read what it says
   it would send, then run `cargo publish`.
4. Go to the crate's Settings, then Trusted Publishing, and add a publisher
   with these values.

   | Field               | Value             |
   | ------------------- | ----------------- |
   | Repository owner    | `AndreKoraleski`  |
   | Repository name     | `csexpr`          |
   | Workflow filename   | `release.yml`     |
   | Environment         | `crates-io`       |

5. Revoke the token at <https://crates.io/settings/tokens>. Nothing needs it
   again, and a token that does not exist cannot leak.

### PyPI

PyPI does have pending publishers, so the name can be claimed before anything
is uploaded and the first wheels can go up by tag like the rest.

Go to <https://pypi.org/manage/account/publishing/>, choose GitHub, and fill
in these values.

| Field             | Value            |
| ----------------- | ---------------- |
| PyPI Project Name | `csexpr`         |
| Owner             | `AndreKoraleski` |
| Repository name   | `csexpr`         |
| Workflow name     | `release.yml`    |
| Environment name  | `pypi`           |

The project appears on PyPI the first time the workflow uploads to it.

### GitHub

The two publishing jobs run in environments named `crates-io` and `pypi`,
which is what the Environment field above refers to. GitHub makes an
environment the first time a job asks for one, so nothing has to be done here
for a release to work.

Settings, then Environments, is where to add a required reviewer if a publish
should wait for a person to approve it. That is worth doing for a crate whose
output gets signed, and it costs one click per release.

## Every release

1. Move the entries under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md)
   under a new `## [X.Y.Z]` heading, and add the comparison link at the foot
   of the file next to the others.
2. Raise `version` under `[workspace.package]` in [Cargo.toml](Cargo.toml).
   That is the only place it is written. The library and the Python bindings
   both read it from there, and the Python package takes its version from the
   bindings.
3. Run the checks, or let a push to `master` run them.

   ```sh
   cargo test --all-targets
   cargo test --doc
   ```

4. Commit it as `chore: release X.Y.Z`.
5. Tag it and push both.

   ```sh
   git tag vX.Y.Z
   git push origin master vX.Y.Z
   ```

## What the tag sets off

The workflow refuses the tag unless the version in the manifest, the version
the tag names, and a heading in the changelog all agree, and unless the tests
pass. After that it publishes the crate, builds a wheel on Linux, macOS and
Windows along with a source distribution, uploads them to PyPI, and drafts the
release notes on GitHub.

One wheel per platform serves every Python from 3.10 up, since the extension
is built against the stable ABI.

## When something fails halfway

Both publishing steps ask first whether the version is already there and do
nothing if it is, so a failed run can be run again from the Actions tab
without tripping over what it already did.

What cannot be undone is a version that went up wrong. Neither registry lets a
file be replaced once uploaded. crates.io can only yank, with
`cargo yank --version X.Y.Z`, which leaves existing lockfiles working and
keeps new ones from choosing it. PyPI can only delete, which does not free the
filename. Either way the fix is to release the next patch version rather than
to try to correct the one that went out.

## Rehearsing

`cargo publish --dry-run` builds what would be sent, and `cargo package
--list` prints the files it would contain. On the Python side, `maturin build
--release` from `bindings/python` builds a wheel for the machine it runs on.
