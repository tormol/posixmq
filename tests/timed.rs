//! Tests for _timeout() and _deadline() methods

use std::io::ErrorKind;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

extern crate posixmq;
use posixmq::{remove_queue, OpenOptions, PosixMq};

/// Create smaller queues to avoid running into the total queues size limit.
/// Several of the tests also require a capacity of 1.
fn tmp_mq(name: &str) -> PosixMq {
    let mq = OpenOptions::readwrite()
        .capacity(1)
        .max_msg_len(64)
        .create()
        .open(name)
        .expect(&format!("cannot open or create {}", name));
    let _ = remove_queue(name);
    mq
}

#[test]
fn timeout_doesnt_block_indefinitely() {
    let mq = tmp_mq("/timed_send_and_receive");
    let err = mq
        .recv_timeout(&mut [0; 64], Duration::from_millis(10))
        .unwrap_err()
        .kind();
    assert_eq!(err, ErrorKind::TimedOut);
    mq.send_timeout(1, b"1", Duration::from_millis(10)).unwrap();
    let err = mq
        .send_timeout(2, b"2", Duration::from_millis(10))
        .unwrap_err()
        .kind();
    assert_eq!(err, ErrorKind::TimedOut);
    mq.recv_timeout(&mut [0; 64], Duration::from_millis(10))
        .unwrap();
}

#[test]
fn subsecond_timeouts_matter() {
    let mq = Arc::new(tmp_mq("/tiny_timeouts_matter"));
    let long_mq = mq.clone();
    let long =
        thread::spawn(move || long_mq.recv_timeout(&mut [0; 64], Duration::from_millis(500)));
    let short_mq = mq.clone();
    let short =
        thread::spawn(move || short_mq.recv_timeout(&mut [0; 64], Duration::from_millis(100)));
    thread::sleep(Duration::from_millis(300));
    mq.send(0, b"patience is a virtue").unwrap();
    assert_eq!(
        short.join().unwrap().unwrap_err().kind(),
        ErrorKind::TimedOut
    );
    long.join()
        .unwrap()
        .expect("both sub-second receives failed");
}

#[test]
fn fullsecond_timeouts_matter() {
    let mq = Arc::new(tmp_mq("/big_timeouts_matter"));
    let long_mq = mq.clone();
    let long = thread::spawn(move || long_mq.recv_timeout(&mut [0; 64], Duration::from_secs(2)));
    let short_mq = mq.clone();
    let short =
        thread::spawn(move || short_mq.recv_timeout(&mut [0; 64], Duration::new(1, 100_000_000)));
    thread::sleep(Duration::from_millis(1_550));
    mq.send(0, b"patience is a virtue").unwrap();
    assert_eq!(
        short.join().unwrap().unwrap_err().kind(),
        ErrorKind::TimedOut
    );
    long.join()
        .unwrap()
        .expect("both full-second receives failed");
}

#[test]
fn deadline() {
    let mq = tmp_mq("/deadline");
    let later = SystemTime::now() + Duration::from_secs(1);
    assert_eq!(
        mq.recv_deadline(&mut [0; 64], later).unwrap_err().kind(),
        ErrorKind::TimedOut
    );
    // returns immediately after the previous call timed out.
    assert_eq!(
        mq.recv_deadline(&mut [0; 64], later).unwrap_err().kind(),
        ErrorKind::TimedOut
    );
    let later = SystemTime::now() + Duration::from_secs(1);
    mq.send_deadline(0, b"I won't wait *forever*", later)
        .unwrap();
    let result = mq.send_deadline(0, b"I won't wait *forever*", later);
    assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
}

#[test]
fn expired() {
    let mq = tmp_mq("/expired");
    let result = mq.recv_timeout(&mut [0; 64], Duration::from_nanos(0));
    assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
    let result = mq.recv_deadline(&mut [0; 64], SystemTime::now() - Duration::new(10, 10));
    assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
    // now() expires before the syscall is made
    mq.send_deadline(0, b"no time but space", SystemTime::now())
        .unwrap();
    let result = mq.send_deadline(0, b"no time no space", SystemTime::now());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
}

#[test]
fn ignored_when_nonblocking() {
    let mq = OpenOptions::readwrite()
        .nonblocking()
        .capacity(1)
        .max_msg_len(64)
        .create()
        .open("/deadline_is_now")
        .unwrap();
    let _ = remove_queue("/deadline_is_now");
    let later = SystemTime::now() + Duration::from_secs(60);
    let result = mq.recv_deadline(&mut [0; 64], later);
    assert_eq!(result.unwrap_err().kind(), ErrorKind::WouldBlock);
    mq.send_deadline(0, b"I can't wait", later).unwrap();
    let result = mq.send_deadline(0, b"I can't wait", later);
    assert_eq!(result.unwrap_err().kind(), ErrorKind::WouldBlock);
}

#[test]
fn bad_timeouts() {
    let mq = tmp_mq("/time_overflow");
    if cfg!(all(target_os = "linux", target_arch = "i686")) {
        // has 32bit time_t
        let result = mq.send_timeout(0, b"zzz", Duration::new(i32::max_value() as u64, 0));
        assert_eq!(
            result.expect_err("32bit overflow").kind(),
            ErrorKind::InvalidInput
        );
    }
    let result = mq.send_timeout(0, b"zzz", Duration::new(!0, 999_999_999));
    assert_eq!(
        result.expect_err("complete wraparound").kind(),
        ErrorKind::InvalidInput
    );
    let result = mq.send_timeout(0, b"zzz", Duration::new(!0 - 100, 0));
    assert_eq!(result.expect_err("too big").kind(), ErrorKind::InvalidInput);
    let result = mq.send_timeout(0, b"zzz", Duration::new(i64::max_value() as u64 - 100, 0));
    assert_eq!(
        result.expect_err("overflow").kind(),
        ErrorKind::InvalidInput
    );
}

#[test]
fn negative_deadline() {
    // SystemTime currently has the same range as time_t, so
    // unconvertable values aren't possible.
    // While Linux and DragonFly BSD rejects pre-epoch values with EINVAL,
    // other OSes support them.
    let mq = tmp_mq("/negative_deadline");
    let error = if cfg!(any(target_os = "linux", target_os = "dragonfly")) {
        ErrorKind::InvalidInput
    } else {
        ErrorKind::TimedOut
    };
    let no_fraction = SystemTime::UNIX_EPOCH - Duration::from_secs(365 * 24 * 60 * 60);
    let result = mq.recv_deadline(&mut [0; 64], no_fraction);
    assert_eq!(result.expect_err("negative deadline").kind(), error);
    let only_fraction = SystemTime::UNIX_EPOCH - Duration::new(0, 100_000_000);
    let result = mq.recv_deadline(&mut [0; 64], only_fraction);
    assert_eq!(result.expect_err("negative deadline").kind(), error);
    let with_fraction = SystemTime::UNIX_EPOCH - Duration::new(10, 999_999_999);
    let result = mq.recv_deadline(&mut [0; 64], with_fraction);
    assert_eq!(result.expect_err("negative deadline").kind(), error);
}
