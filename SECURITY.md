# Security policy

## Reporting

Report anything that looks like a vulnerability through [private vulnerability
reporting] on this repository, under Security, rather than in a public issue.
An initial reply should come within a week.

[private vulnerability reporting]:
  https://github.com/AndreKoraleski/csexpr/security/advisories/new

## What counts

This crate parses input that an application may have received from anywhere,
and RFC 9804 S-expressions carry signed material in SPKI and SDSI, so the
following are all worth reporting.

- A panic, an abort, or a stack overflow reached from `decode`, on any input.
- Memory taken out of proportion to the size of the input.
- Two different S-expressions with the same canonical representation, or one
  S-expression with two, since a signature over those octets rests on there
  being exactly one.
- Input accepted by `decode::parse_canonical` that is not what
  `encode::canonical` would write for the value it read.
- A restriction of `decode::Parser` that lets through what it says it refuses.

Nesting deeper than the parser was told to accept is not one of these. A parser
refuses it, and the depth it accepts by default is bounded so that the tree it
builds can be dropped, cloned and compared safely. Raising `max_depth` far past
that default gives the hazard back, which the documentation says plainly.

## Supported versions

The latest release is the supported one, while the crate is before 1.0.
