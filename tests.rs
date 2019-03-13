use std::io::ErrorKind;

extern crate posixmq;
use posixmq::{PosixMq, OpenOptions, unlink, unlink_c, name_from_bytes};

#[cfg(feature="mio")]
extern crate mio;

#[test]
fn unlink_invalid_names() {
    assert_eq!(unlink("").unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(unlink("/").unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(unlink("//").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(unlink("/foo/bar").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(unlink("/foo/").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(unlink("//").unwrap_err().kind(), ErrorKind::PermissionDenied);
    //assert_eq!(unlink("/root").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(unlink("/foo\0").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(unlink(&vec![b'a'; 1000]).unwrap_err().kind(), ErrorKind::Other);
}

#[test]
fn open_invalid_names() {
    assert_eq!(PosixMq::create("").unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(PosixMq::create("/").unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(PosixMq::create("//").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(PosixMq::create("/foo/bar").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(PosixMq::create("/foo/").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(PosixMq::create("//").unwrap_err().kind(), ErrorKind::PermissionDenied);
    //assert_eq!(PosixMq::create("/root").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(PosixMq::create("/foo\0").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create(&vec![b'a'; 1000]).unwrap_err().kind(), ErrorKind::Other);
}

#[test]
fn reserved_names() {
    // That /. and /.. cannot be created is not that surprising, but not documented either.
    assert_eq!(PosixMq::create("/.").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(PosixMq::create("/..").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(PosixMq::open("/.").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(PosixMq::open("/..").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(unlink("/.").unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(unlink("/..").unwrap_err().kind(), ErrorKind::PermissionDenied);
}

#[test]
fn invalid_cstr_names() {
    use std::ffi::CStr;
    fn c(s: &str) -> &CStr {
        CStr::from_bytes_with_nul(s.as_bytes()).expect("_c name must have trailing '\0'")
    }

    assert_eq!(unlink_c(c("\0")).unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(unlink_c(c("/\0")).unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(
        OpenOptions::readonly().create().open_c(c("\0")).unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    assert_eq!(
        OpenOptions::readonly().create().open_c(c("/\0")).unwrap_err().kind(),
        ErrorKind::NotFound
    );
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
fn create_and_remove() {
    let mq = OpenOptions::readwrite().create_new().open("/flash");
    assert!(mq.is_ok());
    assert!(unlink("/flash").is_ok());
    assert_eq!(PosixMq::open("/flash").unwrap_err().kind(), ErrorKind::NotFound);
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

    assert!(nb.send(9, b"a").is_ok());
    assert!(bl.send(0, b"").is_ok());
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
        .open("receive").unwrap();
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

    mq.send(20, b"aaaa").unwrap();
    mq.send(40, b"bbb").unwrap();
    mq.send(10, b"cc").unwrap();
    mq.send(30, b"d").unwrap();

    let mut buf = [0; 8192];
    assert_eq!(mq.receive(&mut buf).unwrap(), (40, 3));
    assert_eq!(&buf[..3], b"bbb");
    assert_eq!(mq.receive(&mut buf).unwrap(), (30, 1));
    assert_eq!(&buf[..3], b"dbb");
    assert_eq!(mq.receive(&mut buf).unwrap(), (20, 4));
    assert_eq!(&buf[..4], b"aaaa");
    assert_eq!(mq.receive(&mut buf).unwrap(), (10, 2));
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
    let with_fd = std::net::TcpListener::bind("127.16.13.17:0")
        .expect("cannot listen on any port on loopback");
    assert_eq!(mq_fd, with_fd.as_raw_fd());
}

#[cfg(not(any(target_os="freebsd", target_os="dragonflybsd")))]
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


#[cfg(feature="mio")]
#[test]
fn mio() {
    use mio::{Poll, Events, PollOpt, Ready, Token};

    let mut opts = OpenOptions::readwrite();
    let opts = opts.nonblocking().capacity(1).max_msg_len(10).create_new();
    let mq_a = opts.open("/mio_a").unwrap();
    let mq_b = opts.open("/mio_b").unwrap();
    let _ = unlink("/mio_a");
    let _ = unlink("/mio_b");

    let mut events = Events::with_capacity(8);
    let poll = Poll::new().unwrap();
    poll.register(&mq_a, Token(0), Ready::readable(), PollOpt::edge()).unwrap();
    poll.register(&mq_b, Token(1), Ready::readable(), PollOpt::edge()).unwrap();

    // test readable
    mq_a.send(3, b"mio a a").unwrap();
    poll.poll(&mut events, None).unwrap();
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
