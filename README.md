# posixmq

A wrapper around posix message queues.

```rust
let mq = posixmq::PosixMq::open("/queue")?;
let mut buf = [0; 8192];
loop {
    let (priority, len) = mq.receive(&mut buf)?;
    println!("priority: {:3}, message: {}", priority, str::from_utf8(&buf[..len])?);
}
```

## Supported operating systems

Not all operating systems have posix message queues: Linux and most BSDs have them, but macOS, OpenBSD and Windows doesn't. See the crate documentation for details.

## `mio` integration

On Linux at least, posix message queues support polling and can thereby be used with `mio`.
This feature is not enabled by default; enable it by adding this to Cargo.toml:

```toml
[dependencies]
posixmq = {version="0.1", features="mio"}
```

Also remember to open the message queues in nonblocking mode.

## Differences from `posix_mq` crate

* `send()` and `receive()` borrows byte slices instead of consuming and producing vectors, which avoids unnecessary allocations.
* Optionally integrates with `mio` so the message queues can be polled (Linux-only).
* Is dual-licensed Apache-2.0 and MIT instead of only MIT.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
