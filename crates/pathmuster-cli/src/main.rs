//! Command-line interface for PathMuster.

#![forbid(unsafe_code)]
#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)
)]

fn main() {
    println!("pathmuster {}", pathmuster::version());
}
