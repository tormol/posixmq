use std::ffi::CStr;
use std::io::ErrorKind;

extern crate posixmq;
use posixmq::{PosixMq, OpenOptions, Attributes, unlink, unlink_c, name_from_bytes};

#[cfg(feature="mio")]
extern crate mio;

#[test]
fn name_with_nul() {
    assert_eq!(unlink("/foo\0").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create("/foo\0").unwrap_err().kind(), ErrorKind::InvalidInput);
}

#[test]
fn name_too_long() {
    assert_eq!(unlink(&vec![b'a'; 1000]).unwrap_err().kind(), ErrorKind::Other);
    assert_eq!(PosixMq::create(&vec![b'a'; 1000]).unwrap_err().kind(), ErrorKind::Other);
}

#[cfg(not(any(target_os="netbsd", target_os="dragonflybsd")))] // allown any name
#[test]
fn unlink_invalid_names() {
    assert_eq!(unlink("").unwrap_err().kind(), unlink("/").unwrap_err().kind(), "\"\"");
    let err = unlink("//").unwrap_err().kind();
    assert!(err == ErrorKind::PermissionDenied  ||  err == ErrorKind::InvalidInput, "//");
    let err = unlink("/foo/bar").unwrap_err().kind();
    assert!(err == ErrorKind::PermissionDenied  ||  err == ErrorKind::InvalidInput, "/foo/bar");
    let err = unlink("/foo/").unwrap_err().kind();
    assert!(err == ErrorKind::PermissionDenied  ||  err == ErrorKind::InvalidInput, "/foo/");
    //assert_eq!(unlink("/root").unwrap_err().kind(), ErrorKind::PermissionDenied, "/root");
}

#[cfg(not(any(target_os="netbsd", target_os="dragonflybsd")))] // allown any name
#[test]
fn open_invalid_names() {
    assert_eq!(PosixMq::create("").unwrap_err().kind(), PosixMq::create("/").unwrap_err().kind());
    let err = PosixMq::create("//").expect_err("create //").kind();
    assert!(err == ErrorKind::PermissionDenied  ||  err == ErrorKind::InvalidInput);
    let err = PosixMq::create("/foo/bar").expect_err("create /foo/bar").kind();
    assert!(err == ErrorKind::PermissionDenied  ||  err == ErrorKind::InvalidInput);
    let err = PosixMq::create("/foo/").expect_err("create /foo/").kind();
    assert!(err == ErrorKind::PermissionDenied  ||  err == ErrorKind::InvalidInput);
    //assert_eq!(PosixMq::create("/root").unwrap_err().kind(), ErrorKind::PermissionDenied);
}

// can make FreeBSD kernel panic. NetBSD and DragonFlyBSD accepts any name
#[cfg(not(any(target_os="freebsd", target_os="netbsd", target_os="dragonflybsd")))]
#[test]
fn reserved_names() {
    // That Linux and FreeBSD doesn't like /. and /.. is not that surprising
    // considering that they expose posix message queues through a virtual
    // file system, but not documented either.
    assert_eq!(unlink("/.").unwrap_err().kind(), ErrorKind::PermissionDenied, "unlink /.");
    assert_eq!(unlink("/..").unwrap_err().kind(), ErrorKind::PermissionDenied, "unlink /..");
    assert_eq!(PosixMq::open("/.").unwrap_err().kind(), ErrorKind::PermissionDenied, "open /.");
    assert_eq!(PosixMq::open("/..").unwrap_err().kind(), ErrorKind::PermissionDenied, "open /..");
    assert_eq!(PosixMq::create("/.").unwrap_err().kind(), ErrorKind::PermissionDenied, "create /.");
    assert_eq!(PosixMq::create("/..").unwrap_err().kind(), ErrorKind::PermissionDenied, "create /..");
}

#[test]
fn empty_cstr_name() {
    let empty = CStr::from_bytes_with_nul(b"\0").unwrap();
    let err = unlink_c(empty).expect_err("unlink empty").kind();
    assert!(err == ErrorKind::InvalidInput || err == ErrorKind::NotFound, "unlink empty");
    let err = OpenOptions::readonly().create().open_c(empty).expect_err("create empty").kind();
    assert_eq!(err, ErrorKind::InvalidInput, "create empty");
}

#[cfg(not(any(target_os="netbsd", target_os="dragonflybsd")))] // allown any name
#[test]
fn invalid_cstr_names() {
    use std::ffi::CStr;
    fn c(s: &str) -> &CStr {
        CStr::from_bytes_with_nul(s.as_bytes()).expect("_c name must have trailing '\0'")
    }

    let err = unlink_c(c("/\0")).expect_err("unlink /").kind();
    assert!(err == ErrorKind::InvalidInput || err == ErrorKind::NotFound, "unlink /");
    let err = unlink_c(c("noslash\0")).expect_err("unlink noslash").kind();
    assert!(err == ErrorKind::InvalidInput || err == ErrorKind::NotFound, "unlink noslash");
    let err = OpenOptions::readonly().create().open_c(c("noslash\0")).expect_err("create noslash");
    assert_eq!(err.kind(), ErrorKind::InvalidInput, "create noslash");
    let err = OpenOptions::readonly().create().open_c(c("/\0")).expect_err("create /").kind();
    assert!(err == ErrorKind::NotFound  ||  err == ErrorKind::InvalidInput, "create /");
}

#[test]
fn nonexistant() {
    let _ = unlink(b"/404"); // in case it already exists
    assert_eq!(unlink("/404").unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(PosixMq::open("/404").unwrap_err().kind(), ErrorKind::NotFound);
}

#[test]
fn open_invalid_params() {
    // won't get an error for invalid permissions, beecause unknown bits are ignored
    assert_eq!(
        OpenOptions::readwrite().capacity(2).max_msg_len(0).create().open("zero_len").unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    assert_eq!(
        OpenOptions::readwrite().capacity(0).max_msg_len(2).create().open("zero_cap").unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    assert_eq!(
        OpenOptions::readwrite().capacity(2).max_msg_len(!0).create().open("bad_len").unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    assert_eq!(
        OpenOptions::readwrite().capacity(!0).max_msg_len(2).create().open("bad_cap").unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
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
fn is_cloexec() {
    let mq = PosixMq::create("/is_cloexec").unwrap();
    let _ = posixmq::unlink("/is_cloexec");
    assert!(mq.is_cloexec());
}

#[test]
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
fn change_cloexec() {
    let mq = PosixMq::create("/change_cloexec").unwrap();
    let _ = posixmq::unlink("/change_cloexec");
    unsafe { mq.set_cloexec(false).unwrap() };
    assert!(!mq.is_cloexec());
    unsafe { mq.set_cloexec(true).unwrap() };
    assert!(mq.is_cloexec());
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
fn drop_closes() {
    use std::os::unix::io::AsRawFd;

    // brittle!
    let mq = PosixMq::create("/close").unwrap();
    let _ = unlink("/close");

    let mq_fd = mq.as_raw_fd();
    drop(mq);
    // Would ideally have created something completely unnamed,
    // but this should also be unobtrusive.
    let with_fd = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("cannot listen on any port on loopback");
    assert_eq!(mq_fd, with_fd.as_raw_fd());
}

#[cfg(not(target_os="freebsd"))]
#[test]
fn into_fd_doesnt_drop() {
    use std::os::unix::io::{FromRawFd, IntoRawFd};

    unsafe {
        let mq = PosixMq::create("/via_fd").unwrap();
        let _ = unlink("/via_fd");

        mq.send(0, b"foo").unwrap();
        let fd = mq.into_raw_fd();
        let mq = PosixMq::from_raw_fd(fd);
        assert_eq!(mq.receive(&mut[0; 8192]).unwrap(), (0, 3));
    }
}

#[test]
fn is_send_and_sync() {
    fn is_send<T:Send>() -> bool {true}
    fn is_sync<T:Sync>() -> bool {true}
    is_send::<PosixMq>();
    is_sync::<PosixMq>();
}


#[cfg(all(feature="mio", not(target_os="netbsd")))]
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
