# posixmq

A Rust library for working with [posix message queues](https://linux.die.net/man/7/mq_overview).

[![crates.io](https://img.shields.io/crates/v/posixmq.svg)](https://crates.io/crates/posixmq) [![Build Status](https://api.cirrus-ci.com/github/tormol/posixmq.svg)](https://cirrus-ci.com/github/tormol/posixmq) [![Documentation](https://docs.rs/posixmq/badge.svg)](https://docs.rs/posixmq/)

```rust
let mq = posixmq::PosixMq::open("/queue")?;
let mut buf = vec![0; mq.attributes().max_msg_size];
loop {
    let (priority, len) = mq.receive(&mut buf)?;
    println!("priority: {:3}, message: {}", priority, str::from_utf8(&buf[..len])?);
}
```

## Supported operating systems

Not all operating systems have posix message queues: Linux and most BSDs have them, but macOS, OpenBSD and Windows doesn't. See the crate documentation for details.

This library is **tested to work on Linux and FreeBSD, but **has issues on NetBSD**.

## mio integration

On Linux and FreeBSD posix message queues can be polled, and therefore used with [mio](https://github.com/carllerche/mio). This feature is not enabled by default; enable it in Cargo.toml with:

```toml
[dependencies]
posixmq = {version="0.1", features=["mio"]}
```

Also remember to open the message queues in nonblocking mode.

## Differences from [posix_mq](https://github.com/aprilabank/posix_mq.rs)

* `send()` and `receive()` borrows byte slices instead of consuming and producing vectors, which avoids unnecessary allocations.
* Optionally integrates with `mio` so the message queues can be polled.
* Is dual-licensed Apache-2.0 and MIT instead of only MIT.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
