//! What reading an S-expression costs.

mod common;

use csexpr::encode::{advanced, canonical, transport};
use csexpr::{Sexp, decode};
use divan::{Bencher, black_box};

/// Counting allocations along with the time, since reading is mostly a matter
/// of how much it copies.
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

/// Reads the canonical representation of each shape (§6.2).
#[divan::bench(args = SHAPES)]
fn canonical(bencher: Bencher, shape: &Shape) {
    let input = canonical::to_vec(&(shape.build)());

    bencher.bench(|| decode::parse_canonical(black_box(&input)));
}

/// Reads the advanced representation of each shape (§6.4).
#[divan::bench(args = SHAPES)]
fn advanced(bencher: Bencher, shape: &Shape) {
    let input = advanced::to_vec(&(shape.build)());

    bencher.bench(|| decode::parse(black_box(&input)));
}

/// Reads the basic transport representation of each shape (§6.3).
#[divan::bench(args = SHAPES)]
fn transport(bencher: Bencher, shape: &Shape) {
    let input = transport::to_vec(&(shape.build)());

    bencher.bench(|| decode::parse(black_box(&input)));
}

/// An S-expression to read, and the name to report it under.
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
