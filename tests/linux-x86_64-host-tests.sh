#!/bin/sh
# Run tests on multiple sub-architectures and Rust versions,
# and check other OSes and architectures
set -e
export RUST_BACKTRACE=1

rm Cargo.lock 2> /dev/null || true
cargo +nightly test -Z minimal-versions --all-features --no-fail-fast -- --quiet
rm Cargo.lock
cargo +1.31.1 test --features mio_06 --no-fail-fast -- --quiet
cargo +1.31.1 test --features mio_06 -- --ignored --test-threads 1 --quiet
cargo +1.39.0 build --features mio_07 --tests --examples
cargo +1.39.0 test --all-features --no-fail-fast -- --quiet
cargo +1.39.0 test --all-features -- --ignored --test-threads 1 --quiet
cargo test --target i686-unknown-linux-gnu --all-features --no-fail-fast -- --quiet
cargo test --target x86_64-unknown-linux-gnux32 --release --all-features --no-fail-fast -- --quiet
cargo test --target x86_64-unknown-linux-musl --all-features -- --quiet
cargo check --target x86_64-unknown-netbsd --tests --examples --all-features
cargo check --target x86_64-unknown-freebsd --tests --examples --all-features
cargo check --target x86_64-sun-solaris --tests --examples # mio not (yet) supported
cargo check --target aarch64-unknown-linux-gnu --tests --examples --all-features
cargo check --target arm-unknown-linux-gnueabi --tests --examples --all-features
