use std::ffi::CStr;
#[cfg(target_os="linux")]
use std::fs;
use std::io::ErrorKind;
#[cfg(target_os="linux")]
use std::os::unix::fs::PermissionsExt;

extern crate posixmq;
use posixmq::{PosixMq, OpenOptions, unlink, unlink_c};

fn cstr(s: &str) -> &CStr {
    CStr::from_bytes_with_nul(s.as_bytes()).expect("_c name must have trailing '\0'")
}

#[test]
fn name_too_long() {
    assert_eq!(unlink(&vec![b'a'; 1000]).unwrap_err().kind(), ErrorKind::Other);
    assert_eq!(PosixMq::create(&vec![b'a'; 1000]).unwrap_err().kind(), ErrorKind::Other);
}

#[cfg(not(any(target_os="netbsd", target_os="dragonfly")))] // allown any name
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

#[cfg(not(any(
    target_os="netbsd", target_os="dragonfly", // allows any name
    target_os="illumos", target_os="solaris", // allows this
)))]
#[test]
fn just_slash() {
    let error = if cfg!(target_os="freebsd") {ErrorKind::InvalidInput} else {ErrorKind::NotFound};
    assert_eq!(unlink("/").expect_err("unlink /").kind(), error, "unlink /");
    assert_eq!(PosixMq::open("/").expect_err("open /").kind(), error, "open /");
    assert_eq!(PosixMq::create("/").expect_err("create /").kind(), error, "create /");
    assert_eq!(unlink_c(cstr("/\0")).expect_err("unlink_c /").kind(), error, "unlink_c /");
    assert_eq!(
        OpenOptions::readonly().create().open_c(cstr("/\0")).expect_err("create_c /").kind(),
        error,
        "create_c /"
    );
}

#[cfg(not(any(target_os="netbsd", target_os="dragonfly")))] // allown any name
#[test]
fn open_invalid_names() {
    let err = PosixMq::create("//").expect_err("create //").kind();
    assert!(err == ErrorKind::PermissionDenied  ||  err == ErrorKind::InvalidInput);
    let err = PosixMq::create("/foo/bar").expect_err("create /foo/bar").kind();
    assert!(err == ErrorKind::PermissionDenied  ||  err == ErrorKind::InvalidInput);
    let err = PosixMq::create("/foo/").expect_err("create /foo/").kind();
    assert!(err == ErrorKind::PermissionDenied  ||  err == ErrorKind::InvalidInput);
}

#[cfg(not(any(
    target_os="netbsd", target_os="dragonfly", // allows any name
    target_os="illumos", target_os="solaris", // allows these
    target_os="freebsd" // can cause kernel panic
)))]
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
fn cstr_empty_name() {
    let error = if cfg!(target_os="netbsd") || cfg!(target_os="dragonfly") {
        ErrorKind::NotFound
    } else {
        ErrorKind::InvalidInput
    };
    assert_eq!(unlink_c(cstr("\0")).unwrap_err().kind(), error, "unlink");
    assert_eq!(OpenOptions::readonly().open_c(cstr("\0")).unwrap_err().kind(), error, "open");
    assert_eq!(
        OpenOptions::readonly().create().open_c(cstr("\0")).unwrap_err().kind(),
        ErrorKind::InvalidInput,
        "create"
    );
}

#[cfg(not(any(target_os="netbsd", target_os="dragonfly")))] // allown any name
#[test]
fn cstr_no_slash() {
    let noslash = cstr("noslash\0");
    let err = unlink_c(noslash).expect_err("unlink noslash").kind();
    assert!(err == ErrorKind::InvalidInput || err == ErrorKind::NotFound, "unlink noslash");
    let err = OpenOptions::readonly().create().open_c(noslash).expect_err("create noslash").kind();
    assert_eq!(err, ErrorKind::InvalidInput, "create noslash");
}


#[test]
fn open_invalid_params() {
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
fn invalid_permissions() {
    let result = OpenOptions::readwrite().mode(!0).create().open("/set_everything");
    let _ = unlink("/set_everything");
    // All tested operating systems ignore unknown bits
    result.expect("unknown bits or permissions were not ignored");
}

#[cfg(target_os="linux")]
#[test]
fn default_permissions() {
    let _ = PosixMq::create("/default_permissions").unwrap();
    let metadata = fs::metadata("/dev/mqueue/default_permissions").unwrap();
    let _ = unlink("/default_permissions");
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
}

#[cfg(target_os="linux")]
#[test]
fn custom_permissions() {
    let _ = OpenOptions::writeonly()
        .mode(0o344) // bits unlikely to be affected by umask
        .create()
        .open("/custom_permissions")
        .unwrap();
    let metadata = fs::metadata("/dev/mqueue/custom_permissions").unwrap();
    let _ = unlink("/custom_permissions");
    assert_eq!(metadata.permissions().mode() & 0o777, 0o344);
}
