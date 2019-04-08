#!/bin/sh
# Run tests on multiple sub-architectures and Rust versions,
# and check other OSes and architectures

rm Cargo.lock 2> /dev/null
cargo +nightly test -Z minimal-versions --all-features -- --quiet && \
rm Cargo.lock && \
cargo +1.31.1 test --all-features -- --quiet && \
cargo +1.31.1 test --all-features -- --ignored --test-threads 1 --quiet && \
cargo test --target i686-unknown-linux-gnu --all-features -- --quiet && \
cargo test --target x86_64-unknown-linux-gnux32 --release --all-features -- --quiet && \
cargo check --target x86_64-unknown-netbsd --all-features --tests && \
cargo check --target x86_64-unknown-freebsd --all-features --tests && \
cargo check --target aarch64-unknown-linux-gnu --all-features --tests && \
cargo check --target arm-unknown-linux-gnueabi --all-features --tests && \
true
