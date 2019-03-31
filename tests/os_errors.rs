use std::ffi::CStr;
use std::io::ErrorKind;

extern crate posixmq;
use posixmq::{PosixMq, OpenOptions, unlink, unlink_c};

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
fn open_invalid_params() {
    // won't get an error for invalid permissions (on Linux), beecause unknown bits are ignored
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
