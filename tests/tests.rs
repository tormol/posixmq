use std::io::ErrorKind;

extern crate posixmq;
use posixmq::{PosixMq, OpenOptions, Attributes, unlink, name_from_bytes};

#[cfg(feature="mio")]
extern crate mio;

#[test]
fn name_with_nul() {
    assert_eq!(unlink("/foo\0").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create("/foo\0").unwrap_err().kind(), ErrorKind::InvalidInput);
}

#[test]
fn nonexistant() {
    let _ = unlink(b"/404"); // in case it already exists
    assert_eq!(unlink("/404").unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(PosixMq::open("/404").unwrap_err().kind(), ErrorKind::NotFound);
}

#[test]
fn open_custom_capacities() {
    let mq = OpenOptions::readonly()
        .capacity(2)
        .max_msg_len(100)
        .create()
        .open(b"/custom_capacities")
        .unwrap();
    let _ = posixmq::unlink(b"/custom_capacities");
    assert_eq!(mq.attributes(), Attributes {
        capacity: 2,
        max_msg_len: 100,
        current_messages: 0,
        nonblocking: false,
    });
}

#[test]
fn create_and_remove() {
    let mq = OpenOptions::readwrite().create_new().open("/flash");
    assert!(mq.is_ok());
    assert!(unlink("/flash").is_ok());
    assert_eq!(PosixMq::open("/flash").unwrap_err().kind(), ErrorKind::NotFound);
}


#[test]
fn is_not_nonblocking() {
    let mq = PosixMq::create("/is_not_nonblocking").unwrap();
    let _ = posixmq::unlink("/is_not_nonblocking");
    assert!(!mq.is_nonblocking());
    // TODO test that send and receive blocks?
}

#[test]
fn is_nonblocking() {
    let mq = OpenOptions::readwrite()
        .nonblocking()
        .capacity(1)
        .max_msg_len(1)
        .create()
        .open("/is_nonblocking")
        .unwrap();
    let _ = posixmq::unlink("/is_nonblocking");

    assert!(mq.is_nonblocking());
    assert_eq!(mq.receive(&mut[0]).unwrap_err().kind(), ErrorKind::WouldBlock);
    mq.send(5, b"e").unwrap();
    assert_eq!(mq.send(6, b"f").unwrap_err().kind(), ErrorKind::WouldBlock);
}

#[test]
fn change_nonblocking() {
    let mq = PosixMq::create("/change_nonblocking").unwrap();
    let _ = posixmq::unlink("/change_nonblocking");
    mq.set_nonblocking(true).unwrap();
    assert!(mq.is_nonblocking());
    assert_eq!(mq.receive(&mut[0; 8192]).unwrap_err().kind(), ErrorKind::WouldBlock);
    mq.set_nonblocking(false).unwrap();
    assert!(!mq.is_nonblocking());
}

#[test]
#[cfg(not(any(target_os="illumos", target_os="solaris")))]
fn is_cloexec() {
    let mq = PosixMq::create("/is_cloexec").unwrap();
    let _ = posixmq::unlink("/is_cloexec");
    assert!(mq.is_cloexec());
}

#[test]
#[cfg(not(any(target_os="illumos", target_os="solaris")))]
fn is_not_cloexec() {
    let mq = OpenOptions::writeonly()
        .not_cloexec()
        .create()
        .open("/not_cloexec")
        .unwrap();
    let _ = posixmq::unlink("/not_cloexec");
    assert!(!mq.is_cloexec());
}

#[test]
#[cfg(not(any(target_os="illumos", target_os="solaris")))]
fn change_cloexec() {
    let mq = PosixMq::create("/change_cloexec").unwrap();
    let _ = posixmq::unlink("/change_cloexec");
    unsafe { mq.set_cloexec(false).unwrap() };
    assert!(!mq.is_cloexec());
    unsafe { mq.set_cloexec(true).unwrap() };
    assert!(mq.is_cloexec());
}


#[test]
#[cfg(not(any(target_os="freebsd", target_os="illumos", target_os="solaris")))]
fn try_clone() {
    use std::os::unix::io::AsRawFd;

    let a = PosixMq::create("/clone_fd").unwrap();
    let _ = posixmq::unlink("/clone_fd");
    let b = a.try_clone().unwrap();
    assert!(a.as_raw_fd() != b.as_raw_fd());
    assert!(b.is_cloexec());
    a.send(0, b"a").expect("original descriptor should not be closed");
    b.send(1, b"b").expect("cloned descriptor is should be usable");
    assert_eq!(a.attributes().current_messages, 2, "descriptors should point to the same queue");
    drop(a);
    b.send(2, b"c").expect("cloned descriptor should work after closing the original");
}


#[test]
fn send_errors() {
    let _ = unlink("send");
    let nb = OpenOptions::writeonly()
        .nonblocking()
        .create()
        .max_msg_len(1)
        .capacity(2)
        .open("send")
        .unwrap();
    assert_eq!(nb.send(0, b"too long").unwrap_err().kind(), ErrorKind::Other);

    let bl = OpenOptions::readwrite().open("send").unwrap();
    assert_eq!(bl.send(!0, b"f").unwrap_err().kind(), ErrorKind::InvalidInput);

    nb.send(9, b"a").expect("nonblocking send \"a\"");
    if cfg!(target_os="netbsd") {// NetBSD doesn't allow empty messages
        bl.send(0, b"b").expect("blocking send \"b\"");
    } else {
        bl.send(0, b"").expect("blocking send empty");
    }
    assert_eq!(nb.send(0, b"b").unwrap_err().kind(), ErrorKind::WouldBlock);

    let ro = OpenOptions::readonly().open("send").unwrap();
    assert_eq!(ro.send(0, b"").unwrap_err().kind(), ErrorKind::Other); // opened read-only

    let _ = unlink("send");
}

#[test]
fn receive_errors() {
    let nb = OpenOptions::readonly()
        .nonblocking()
        .create()
        .max_msg_len(1)
        .capacity(2)
        .open("receive")
        .unwrap();
    assert_eq!(nb.receive(&mut[0; 2]).unwrap_err().kind(), ErrorKind::WouldBlock);
    assert_eq!(nb.receive(&mut[]).unwrap_err().kind(), ErrorKind::Other); // buffer too short
    let wo = OpenOptions::writeonly().open("receive").unwrap();
    assert_eq!(wo.receive(&mut[0; 2]).unwrap_err().kind(), ErrorKind::Other); // opened write-only

    let _ = unlink("receive");
}

#[test]
fn send_and_receive() {
    let mq = PosixMq::create(b"/send_and_receive").unwrap();
    let _ = unlink(b"/send_and_receive");

    mq.send(2, b"aaaa").unwrap();
    mq.send(4, b"bbb").unwrap();
    mq.send(1, b"cc").unwrap();
    mq.send(3, b"d").unwrap();

    let mut buf = [0; 8192];
    assert_eq!(mq.receive(&mut buf).unwrap(), (4, 3));
    assert_eq!(&buf[..3], b"bbb");
    assert_eq!(mq.receive(&mut buf).unwrap(), (3, 1));
    assert_eq!(&buf[..3], b"dbb");
    assert_eq!(mq.receive(&mut buf).unwrap(), (2, 4));
    assert_eq!(&buf[..4], b"aaaa");
    assert_eq!(mq.receive(&mut buf).unwrap(), (1, 2));
    assert_eq!(&buf[..4], b"ccaa");
}

#[test]
fn name_from_bytes_normal() {
    use std::borrow::Cow;

    match name_from_bytes("/good\0") {
        Cow::Borrowed(cstr) => assert_eq!(cstr.to_bytes(), b"/good"),
        _ => panic!("should not allocate here"),
    }
    match name_from_bytes(b"/bad") {
        Cow::Owned(cstring) => assert_eq!(cstring.to_bytes(), b"/bad"),
        _ => panic!("should allocate for missing \\0"),
    }
    match name_from_bytes("bad\0") {
        Cow::Owned(cstring) => assert_eq!(cstring.to_bytes(), b"/bad"),
        _ => panic!("should allocate for missing slash"),
    }
    match name_from_bytes("doubly_bad") {
        Cow::Owned(cstring) => assert_eq!(cstring.to_bytes(), b"/doubly_bad"),
        _ => panic!("should allocate for doubly bad"),
    }
    match name_from_bytes(b"") {
        Cow::Owned(cstring) => assert_eq!(cstring.to_bytes(), b"/"),
        _ => panic!("should allocate for empty slice"),
    }
}

#[test]
#[should_panic]
fn name_from_bytes_interior_nul() {
    name_from_bytes("/good\0shit\0");
}

#[test]
#[ignore] // racy
fn drop_closes() {
    let mq = PosixMq::create("/close").unwrap();
    let _ = unlink("/close");
    let mqd = mq.as_raw_mqd();
    let fake_clone = unsafe { PosixMq::from_raw_mqd(mqd) };
    // cast to usize because pointers cannot be compared directly
    assert_eq!(fake_clone.as_raw_mqd() as usize, mqd as usize);
    fake_clone.send(0, b"b").expect("as_raw_mqd() should not close");
    drop(mq);
    fake_clone.send(0, b"b").expect_err("Drop should have closed the descriptor");
    // also tests that drop ignores errors
}

#[cfg(not(any(target_os="freebsd", target_os="illumos", target_os="solaris")))]
#[test]
fn into_fd_doesnt_drop() {
    use std::os::unix::io::{FromRawFd, IntoRawFd};

    let mq = PosixMq::create("/via_fd").unwrap();
    let _ = unlink("/via_fd");
    unsafe {
        mq.send(0, b"foo").unwrap();
        let fd = mq.into_raw_fd();
        let mq = PosixMq::from_raw_fd(fd);
        assert_eq!(mq.receive(&mut[0; 8192]).unwrap(), (0, 3));
    }
}

#[test]
fn int_mqd_doesnt_drop() {
    let mq = PosixMq::create("/raw_mqd").unwrap();
    let _ = unlink("/raw_mqd");
    unsafe {
        let mqd = mq.into_raw_mqd();
        let mq = PosixMq::from_raw_mqd(mqd);
        mq.send(0, b"lorem ipsum").expect("descriptor should remain open");
    }
}

#[test]
fn is_send_and_sync() {
    fn is_send<T:Send>() -> bool {true}
    fn is_sync<T:Sync>() -> bool {true}
    is_send::<PosixMq>();
    is_sync::<PosixMq>();
}


#[cfg(feature="mio")]
#[test]
fn mio() {
    use mio::{Poll, Events, PollOpt, Ready, Token};

    // Start the poll before creating masseage queues so that the syscalls are
    // easier to separate when debugging.
    let mut events = Events::with_capacity(8);
    let poll = Poll::new().expect("cannot create mio Poll");

    let mut opts = OpenOptions::readwrite();
    let opts = opts.nonblocking().capacity(1).max_msg_len(10).create_new();
    let mq_a = opts.open("/mio_a").unwrap();
    let mq_b = opts.open("/mio_b").unwrap();
    let _ = unlink("/mio_a");
    let _ = unlink("/mio_b");

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
    mq_a.receive(&mut[0;10]).unwrap();
    assert_eq!(mq_a.receive(&mut[0;10]).unwrap_err().kind(), ErrorKind::WouldBlock);

    // test reregister & writable
    poll.reregister(&mq_b, Token(1), Ready::writable(), PollOpt::edge()).unwrap();
    poll.poll(&mut events, None).unwrap();
    let mut iter = events.iter();
    assert_eq!(iter.next().unwrap().token(), Token(1));
    assert!(iter.next().is_none());
    // drain & restore readiness
    mq_b.send(10, b"b").unwrap();
    mq_b.receive(&mut[0;10]).unwrap();

    // test deregister
    poll.deregister(&mq_a).unwrap();
    mq_a.send(2, b"2").unwrap();
    poll.poll(&mut events, None).unwrap();
    let mut iter = events.iter();
    assert_eq!(iter.next().unwrap().token(), Token(1));
    assert!(iter.next().is_none());

    poll.deregister(&mq_b).unwrap();
}
