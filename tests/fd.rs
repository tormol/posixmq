//! Tests of file descriptor based features.

extern crate posixmq;
use posixmq::{remove_queue, PosixMq};

use std::io::ErrorKind;

#[test]
fn is_cloexec() {
    let mq = PosixMq::create("/is_cloexec").unwrap();
    let _ = remove_queue("/is_cloexec");
    if cfg!(not(any(target_os = "illumos", target_os = "solaris"))) {
        assert!(mq.is_cloexec().unwrap());
    } else {
        let error = mq.is_cloexec().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Other);
        assert!(error.to_string().contains("not available"));
    }
}

#[test]
#[cfg(not(any(target_os = "illumos", target_os = "solaris")))]
fn change_cloexec() {
    let mq = PosixMq::create("/change_cloexec").unwrap();
    let _ = remove_queue("/change_cloexec");
    mq.set_cloexec(false).unwrap();
    assert!(!mq.is_cloexec().unwrap());
    mq.set_cloexec(true).unwrap();
    assert!(mq.is_cloexec().unwrap());
}

#[test]
#[cfg(not(any(target_os = "freebsd", target_os = "illumos", target_os = "solaris")))]
fn try_clone() {
    use std::os::unix::io::AsRawFd;

    let a = PosixMq::create("/clone_fd").unwrap();
    let _ = remove_queue("/clone_fd");
    let b = a.try_clone().unwrap();
    assert!(a.as_raw_fd() != b.as_raw_fd());
    assert!(b.is_cloexec().unwrap());
    a.send(0, b"a")
        .expect("original descriptor should not be closed");
    b.send(1, b"b")
        .expect("cloned descriptor is should be usable");
    assert_eq!(
        a.attributes()
            .expect("get attributes for cloned descriptor")
            .current_messages,
        2,
        "descriptors should point to the same queue"
    );
    drop(a);
    b.send(2, b"c")
        .expect("cloned descriptor should work after closing the original");
}

#[cfg(not(any(target_os = "freebsd", target_os = "illumos", target_os = "solaris")))]
#[test]
fn into_fd_doesnt_drop() {
    use std::os::unix::io::{FromRawFd, IntoRawFd};

    let mq = PosixMq::create("/via_fd").unwrap();
    let _ = remove_queue("/via_fd");
    unsafe {
        mq.send(0, b"foo").unwrap();
        let fd = mq.into_raw_fd();
        let mq = PosixMq::from_raw_fd(fd);
        assert_eq!(mq.recv(&mut [0; 8192]).unwrap(), (0, 3));
    }
}

#[cfg(feature = "mio_06")]
#[test]
fn mio_06() {
    use mio_06::{Events, Poll, PollOpt, Ready, Token};
    use posixmq::OpenOptions;
    use std::io::ErrorKind;

    // Start the poll before creating masseage queues so that the syscalls are
    // easier to separate when debugging.
    let mut events = Events::with_capacity(8);
    let poll = Poll::new().expect("cannot create mio Poll");

    let mut opts = OpenOptions::readwrite();
    let opts = opts.nonblocking().capacity(1).max_msg_len(10).create_new();
    let mq_a = opts.open("/mio_06_a").unwrap();
    let mq_b = opts.open("/mio_06_b").unwrap();
    let _ = remove_queue("/mio_06_a");
    let _ = remove_queue("/mio_06_b");

    poll.register(&mq_b, Token(1), Ready::readable(), PollOpt::edge())
        .expect("cannot register message queue with poll");
    poll.register(&mq_a, Token(0), Ready::readable(), PollOpt::edge())
        .expect("cannot register message queue with poll");

    // test readable
    mq_a.send(3, b"mio a a").unwrap();
    poll.poll(&mut events, None).expect("cannot poll");
    let mut iter = events.iter();
    assert_eq!(iter.next().unwrap().token(), Token(0));
    assert!(iter.next().is_none());
    // drain readiness
    mq_a.recv(&mut [0; 10]).unwrap();
    assert_eq!(
        mq_a.recv(&mut [0; 10]).unwrap_err().kind(),
        ErrorKind::WouldBlock
    );

    // test reregister & writable
    poll.reregister(&mq_b, Token(1), Ready::writable(), PollOpt::edge())
        .unwrap();
    poll.poll(&mut events, None).unwrap();
    let mut iter = events.iter();
    assert_eq!(iter.next().unwrap().token(), Token(1));
    assert!(iter.next().is_none());
    // drain & restore readiness
    mq_b.send(10, b"b").unwrap();
    mq_b.recv(&mut [0; 10]).unwrap();

    // test deregister
    poll.deregister(&mq_a).unwrap();
    mq_a.send(2, b"2").unwrap();
    poll.poll(&mut events, None).unwrap();
    let mut iter = events.iter();
    assert_eq!(iter.next().unwrap().token(), Token(1));
    assert!(iter.next().is_none());

    poll.deregister(&mq_b).unwrap();
}

#[cfg(feature = "mio_07")]
#[test]
fn mio_07() {
    use mio_07::{Events, Interest, Poll, Token};
    use posixmq::OpenOptions;
    use std::io::ErrorKind;

    // Start the poll before creating masseage queues so that the syscalls are
    // easier to separate when debugging.
    let mut events = Events::with_capacity(8);
    let mut poll = Poll::new().expect("cannot create mio Poll");

    let mut opts = OpenOptions::readwrite();
    let opts = opts.nonblocking().capacity(1).max_msg_len(10).create_new();
    let mq_a = opts.open("/mio_a").unwrap();
    let mq_b = opts.open("/mio_b").unwrap();
    let _ = remove_queue("/mio_a");
    let _ = remove_queue("/mio_b");

    poll.registry()
        .register(&mut &mq_b, Token(1), Interest::READABLE)
        .expect("cannot register message queue with poll");
    poll.registry()
        .register(&mut &mq_a, Token(0), Interest::READABLE)
        .expect("cannot register message queue with poll");

    // test readable
    mq_a.send(3, b"mio a a").unwrap();
    poll.poll(&mut events, None).expect("cannot poll");
    let mut iter = events.iter();
    assert_eq!(iter.next().unwrap().token(), Token(0));
    assert!(iter.next().is_none());
    // drain readiness
    mq_a.recv(&mut [0; 10]).unwrap();
    assert_eq!(
        mq_a.recv(&mut [0; 10]).unwrap_err().kind(),
        ErrorKind::WouldBlock
    );

    // test reregister & writable
    poll.registry()
        .reregister(&mut &mq_b, Token(1), Interest::WRITABLE)
        .unwrap();
    poll.poll(&mut events, None).unwrap();
    let mut iter = events.iter();
    assert_eq!(iter.next().unwrap().token(), Token(1));
    assert!(iter.next().is_none());
    // drain & restore readiness
    mq_b.send(10, b"b").unwrap();
    mq_b.recv(&mut [0; 10]).unwrap();

    // test deregister
    poll.registry().deregister(&mut &mq_a).unwrap();
    mq_a.send(2, b"2").unwrap();
    poll.poll(&mut events, None).unwrap();
    let mut iter = events.iter();
    assert_eq!(iter.next().unwrap().token(), Token(1));
    assert!(iter.next().is_none());

    poll.registry().deregister(&mut &mq_b).unwrap();
}
