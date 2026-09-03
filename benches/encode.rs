//! What writing an S-expression costs.

mod common;

use csexpr::Sexp;
use csexpr::encode::{advanced, canonical, transport};
use divan::{Bencher, black_box};

/// Counting allocations along with the time, since writing is mostly a matter
/// of how much it copies.
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

/// Writes the canonical representation of each shape (§6.2).
#[divan::bench(args = SHAPES)]
fn canonical(bencher: Bencher, shape: &Shape) {
    let sexp = (shape.build)();

    bencher.bench(|| canonical::to_vec(black_box(&sexp)));
}

/// Writes the advanced representation of each shape (§6.4).
#[divan::bench(args = SHAPES)]
fn advanced(bencher: Bencher, shape: &Shape) {
    let sexp = (shape.build)();

    bencher.bench(|| advanced::to_string(black_box(&sexp)));
}

/// Writes the basic transport representation of each shape (§6.3).
#[divan::bench(args = SHAPES)]
fn transport(bencher: Bencher, shape: &Shape) {
    let sexp = (shape.build)();

    bencher.bench(|| transport::to_string(black_box(&sexp)));
}

/// Measures the size of the canonical representation without writing it.
#[divan::bench(args = SHAPES)]
fn canonical_len(bencher: Bencher, shape: &Shape) {
    let sexp = (shape.build)();

    bencher.bench(|| canonical::encoded_len(black_box(&sexp)));
}

/// An S-expression to write, and the name to report it under.
struct Shape {
    name: &'static str,
    build: fn() -> Sexp,
}

impl std::fmt::Display for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

const SHAPES: &[Shape] = &[
    Shape {
        name: "certificate",
        build: common::certificate,
    },
    Shape {
        name: "hinted",
        build: common::hinted,
    },
    Shape {
        name: "wide",
        build: common::wide,
    },
    Shape {
        name: "large_atom",
        build: common::large_atom,
    },
    Shape {
        name: "deep",
        build: common::deep,
    },
];
