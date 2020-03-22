# posixmq

A Rust library for working with [posix message queues](https://linux.die.net/man/7/mq_overview).

[![crates.io](https://img.shields.io/crates/v/posixmq.svg)](https://crates.io/crates/posixmq) [![Build Status](https://api.cirrus-ci.com/github/tormol/posixmq.svg)](https://cirrus-ci.com/github/tormol/posixmq) ![License](https://img.shields.io/crates/l/posixmq.svg) [![Documentation](https://docs.rs/posixmq/badge.svg)](https://docs.rs/posixmq/)

```rust,no_run
let mq = posixmq::PosixMq::open("/queue").expect("cannot open /queue");
let mut buf = vec![0; mq.attributes().max_msg_len];
loop {
    let (priority, len) = mq.recv(&mut buf).expect("recv() failed");
    let msg = std::str::from_utf8(&buf[..len]).expect("not UTF-8");
    println!("priority: {:3}, message: {}", priority, msg);
}
```

## Supported operating systems

posixmq has been tested to work on Linux, FreeBSD, NetBSD, DragonFly BSD and OmniOSce, but not all features are available everywhere. See rustdoc for details.  
***macOS, OpenBSD, Android and Windows doesn't have posix message queues**, and this crate will fail to compile there.

## optional mio integration

On Linux, FreeBSD and DragonFly posix message queues can be registered with epoll / kqueue, and therefore used with [mio](https://github.com/tokio-rs/mio).
Both mio version 0.6 and 0.7 are supported, through the opt-in crate features `mio_06` and `mio_07`.
Enable the feature for the mio version you use in Cargo.toml with for example:

```toml
[dependencies]
mio = {version="0.7", features=["os-poll"]}
posixmq = {version="0.2", features=["mio_07"]}
```

Because of [Cargo bug #4866](https://github.com/rust-lang/cargo/issue/4866) posixmq unintentionally enables the `os-poll` feature of Mio 0.7.  
This will hopefully change, so please enable the feature yourself as show above even if your program will complile without it at the moment. [`cargo -Zfeatures=all`](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#features) can be used to check that one doesn't accidentally depends on some feature.

Also remember to open the message queues in nonblocking mode.

## Differences from the [posix_mq](https://github.com/aprilabank/posix_mq.rs) crate

* `send()` and `recv()` borrows byte slices instead of consuming and producing vectors, avoiding unnecessary allocations.
* Supports deadlines / timeouts.
* Optionally integrates with Mio.
* Is dual-licensed Apache-2.0 and MIT instead of only MIT.

## Minimum supported Rust version

The minimum Rust version for 1.0.\* releases is 1.39.0 if the `mio_07` feature is enabled, and 1.31.1 otherwise.  
Later 1.\*.0 releases might increase this. Until rustup has builds for DragonFly and Illumos, the minimum version will not be increased past what is available in repositories for these operating systems.  
New optional features might require newer Rust versions.
To lock to a minor release, use `posixmq = "1.0.*"` in Cargo.toml, or just copy posixmq.rs into your project.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
