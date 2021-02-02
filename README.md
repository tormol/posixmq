# posixmq

A Rust library for using [posix message queues](http://man7.org/linux/man-pages/man7/mq_overview.7.html).

[![crates.io](https://img.shields.io/crates/v/posixmq.svg)](https://crates.io/crates/posixmq) [![Build Status](https://api.cirrus-ci.com/github/tormol/posixmq.svg)](https://cirrus-ci.com/github/tormol/posixmq) ![License](https://img.shields.io/crates/l/posixmq.svg) [![Documentation](https://docs.rs/posixmq/badge.svg)](https://docs.rs/posixmq/)

```rust,no_run
let mq = posixmq::PosixMq::open("/queue").expect("cannot open /queue");
let mut buf = vec![0; mq.attributes().unwrap_or_default().max_msg_len];
loop {
    let (priority, len) = mq.recv(&mut buf).expect("recv() failed");
    let msg = std::str::from_utf8(&buf[..len]).expect("not UTF-8");
    println!("priority: {:3}, message: {}", priority, msg);
}
```

## Supported Operating Systems

posixmq has been tested to work on Linux, FreeBSD, NetBSD, DragonFly BSD and OmniOSce, but not all features are available everywhere. See rustdoc for details.  
***macOS, OpenBSD, Android and Windows doesn't have posix message queues**, and this crate will fail to compile there.

## Optional mio Integration

On Linux, FreeBSD and DragonFly BSD, posix message queues can be registered with epoll / kqueue, and therefore used with [mio](https://github.com/tokio-rs/mio).
Both mio version 0.6 and 0.7 are supported, through the opt-in crate features `mio_06` and `mio_07`.
Enable the feature for the mio version you use in Cargo.toml with for example:

```toml
[dependencies]
mio = {version="0.7", features=["os-poll"]} # you probably need os-poll
posixmq = {version="1.0", features=["mio_07"]}
```

Also remember to open the message queues in nonblocking mode.

## Minimum supported Rust version

The minimum Rust version for 1.0.\* releases is 1.39.0 if the `mio_07` feature is enabled, and 1.31.1 otherwise.  
Later 1.\*.0 releases might increase this. Until rustup has builds for DragonFly and Illumos, the minimum version will not be increased past what is available in repositories for these operating systems.  
New optional features might require newer Rust versions.
To lock to a minor release, use `posixmq = "1.0.*"` in Cargo.toml, or copy posixmq.rs into your project and remove feature gates as necessary.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Release History

### Version 1.0.0 (2021-02-02)

* Return errors from `.attributes()`,  `.is_nonblocking()` and `.is_cloexec()`.
  For consistency with `std` and to make it easier to catch bugs.
* Rename `.receive()` to `.recv()` for consistency with `std`.
  (`.receive_deadline()` and `.receive_timeout()` are also renamed to `.recv_deadline()` and `.recv_timeout()`)
* Rename `unlink()` to `remove_queue()` for consistency with `std`.
  (`unlink_c()` is also renamed to `remove_queue_c()`)
* Make `Attributes` partially opaque for future extensibility, and implement `Default` for it.
* Remove `name_from_bytes()`.
* Rename `mio` feature for integrating with Mio 0.6 to `mio_06`.
* Add `mio_07` feature for integrating with Mio 0.7.
* Implement `Clone` for the borrowing iterator `posixmq::Iter`.
* Avoid allocating for queue names shorter than 47 bytes.
* Disable non-standard features on unknown operating systems.

### Version 0.2.0 (2019-07-08)

* Change `PosixMq::open()` to open in read-write mode instead of read-only.
* Rename `OpenOptions.permissions()` to `.mode()` for consistency with `std`.
* Remove `OpenOptions.not_cloexec()`.
* Add `.send_timeout()`, `.send_deadline()`, `.receive_timeout()` and `.receive_deadline()`.
* Add `Iter` and `IntoIter` receiving iterator types.

### Version 0.1.1 (2019-04-05)

* Add `.try_clone()`.
* Add `PosixMq::from_raw_mqd()`, `.as_raw_mqd()` and `.into_raw_mqd()`.
* Implement `Sync` on FreeBSD.
* Support DragonFly BSD, NetBSD and Illumos.
* Support ARM targets and `linux-x86_64-unknown-gnux32`.

### Version 0.1.0 (2019-03-25)

* (initial release).
* Support Linux and FreeBSD.
