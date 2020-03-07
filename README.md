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

posixmq has been tested to work on Linux, FreeBSD, NetBSD, DragonFly and OmniOS, but not all features are available everywhere. See rustdoc for details.  
***macOS, OpenBSD and Windows doesn't have posix message queues**, and this crate will fail to compile there.

## optional mio integration

On Linux, FreeBSD and DragonFly posix message queues can be registered with epoll / kqueue, and therefore used with [mio](https://github.com/tokio-rs/mio).
Both mio version 0.6 and 0.7 are supported, through the opt-in crate features `mio_06` and `mio_07`. Enable the integrato with the version you use in Cargo.toml with for example:

```toml
[dependencies]
posixmq = {version="0.2", features=["mio_07"]}
```

Also remember to open the message queues in nonblocking mode.

## Differences from the [posix_mq](https://github.com/aprilabank/posix_mq.rs) crate

* `send()` and `recv()` borrows byte slices instead of consuming and producing vectors, avoiding unnecessary allocations.
* Supports deadlines / timeouts.
* Optionally integrates with `mio`.
* Is dual-licensed Apache-2.0 and MIT instead of only MIT.

## Minimum supported Rust version

The minimum supported Rust version for 1.0.z releases is 1.31.1.  
Later 1.y.0 releases might increase this. Until rustup has builds for DragonFly and Illumos, the minimum version will not be increased past what is available in repositories for these operating systems.
To lock to a minor release, use `posixmq = "1.0.*"` in Cargo.toml, or just copy
posixmq.rs into your project.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
