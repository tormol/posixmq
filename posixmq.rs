/* Copyright 2019 Torbjørn Birch Moltu
 *
 * Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
 * http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
 * http://opensource.org/licenses/MIT>, at your option. This file may not be
 * copied, modified, or distributed except according to those terms.
 */

//! Posix message queue wrapper with optional mio integration.
//!
//! Posix message queues are like pipes, but message-oriented which makes them
//! safe to read by multiple processes. Messages are sorted based on an
//! additional priority parameter. Queues are not placed in the normal file
//! system, but uses a separate, flat namespace. Normal file permissions still
//! apply though.
//! For a longer introduction, see [`man mq_overview`](https://linux.die.net/man/7/mq_overview)
//! or [`man mq`](https://www.unix.com/man-page/netbsd/3/mq/).
//!
//! They are not all that useful, as only Linux and some BSDs implement them,
//! and even there you might be limited to creating queues with a capacity of
//! no more than 10 messages at a time.
//!
//! # Examples
//!
//! Send a couple messages:
//! ```ignore
//! use posixmq::PosixMq;
//!
//! // open the message queue if it exists, or create it if it doesn't.
//! // names should start with a slash and have no more slashes.
//! let mq = PosixMq::create("/hello_posixmq").unwrap();
//! mq.send(0, b"message").unwrap();
//! // messages with equal priority will be received in order
//! mq.send(0, b"queue").unwrap();
//! // but this message has higher priority and will be received first
//! mq.send(10, b"Hello,").unwrap();
//! ```
//!
//! and receive them:
//! ```ignore
//! use posixmq::PosixMq;
//!
//! // open the queue read-only, or fail if it doesn't exist.
//! let mq = PosixMq::open("/hello_posixmq").unwrap();
//! // delete the message queue when you don't need to open it again.
//! // otherwise it will remain until the system is rebooted, consuming
//! posixmq::unlink("/hello_posixmq").unwrap();
//!
//! // the receive buffer must be at least as big as the biggest possible
//! // message, or you will not be allowed to receive anything.
//! let mut buf = vec![0; mq.attributes().max_msg_len];
//! assert_eq!(mq.receive(&mut buf).unwrap(), (10, "Hello,".len()));
//! assert_eq!(mq.receive(&mut buf).unwrap(), (0, "message".len()));
//! assert_eq!(mq.receive(&mut buf).unwrap(), (0, "queue".len()));
//! assert_eq!(&buf[..5], b"queue");
//!
//! // check that there are no more messages
//! assert_eq!(mq.attributes().current_messages, 0);
//! // note that acting on this value is race-prone. A better way to do this
//! // would be to switch our descriptor to non-blocking mode, and check for
//! // an error of type `ErrorKind::WouldBlock`.
//! ```
//!
//! With mio (and `features = ["mio_07"]` in Cargo.toml):
#![cfg_attr(feature="mio_07", doc="```")]
#![cfg_attr(not(feature="mio_07"), doc="```compile_fail")]
//! # extern crate mio_07 as mio;
//! # use mio::{Events, Poll, Interest, Token};
//! # use std::io::ErrorKind;
//! # use std::thread;
//! // set up queue
//! let mut receiver = posixmq::OpenOptions::readonly()
//!     .nonblocking()
//!     .capacity(3)
//!     .max_msg_len(100)
//!     .create_new()
//!     .open("/mio")
//!     .unwrap();
//!
//! // send something from another thread (or process)
//! let sender = thread::spawn(move|| {
//!     let sender = posixmq::OpenOptions::writeonly().open("/mio").unwrap();
//!     posixmq::unlink("/mio").unwrap();
//!     sender.send(0, b"async").unwrap();
//! });
//!
//! // set up mio and register
//! let mut poll = Poll::new().unwrap();
//! poll.registry().register(&mut receiver, Token(0), Interest::READABLE).unwrap();
//! let mut events = Events::with_capacity(10);
//!
//! poll.poll(&mut events, None).unwrap();
//! for event in &events {
//!     if event.token() == Token(0) {
//!         loop {
//!             let mut buf = [0; 100];
//!             match receiver.receive(&mut buf) {
//!                 Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
//!                 Err(e) => panic!("Error receiving message: {}", e),
//!                 Ok((priority, len)) => {
//!                     assert_eq!(priority, 0);
//!                     assert_eq!(&buf[..len], b"async");
//!                 }
//!             }
//!         }
//!     }
//! }
//!
//! sender.join().unwrap();
//! ```
//!
//! See the examples/ directory for more.
//!
//! # Portability
//!
//! While the p in POSIX stands for Portable, that is not a fitting description
//! of their message queues; Support is spotty even among *nix OSes.
//! **Windows, macOS, OpenBSD, Android, ios, Rumprun, Fuchsia and Emscripten
//! doesn't support posix message queues at all.**
//!
//! ## Compatible operating systems and features
//!
//! &nbsp; | Linux | FreeBSD 11+ | NetBSD | DragonFly | Illumos | Solaris
//! -|-|-|-|-|-|-
//! core features | Yes | Yes | Yes | Yes | Yes | Yes | Yes
//! mio `Source` | Yes | Yes | unusable | Yes | No | No | No
//! `FromRawFd`+`IntoRawFd`+`try_clone()` | Yes | No | Yes | Yes | No | No
//! `AsRawFd`+`set_cloexec()` | Yes | Yes | Yes | Yes | No | No
//! Tested? | Yes, CI | Yes, CI | Manually | Manually | Manually | Cross-`check`ed on CI
//!
//! This library will fail to compile if the target OS doesn't have posix
//! message queues.
//!
//! Feature explanations:
//!
//! * `FromRawFd+IntoRawFd`+`try_clone()`: For this to work, the inner `mqd_t`
//!   type must be an `int` typedef, and bad things might happen if it doesn't
//!   represent a file descriptor. These impls are currently on by default and
//!   only disabled when known not to work.
//! * `AsRawFd`+`set_cloexec()`: similar to `FromRawFd` and `IntoRawFd`, but
//!   FreeBSD 11+ has [a function](https://svnweb.freebsd.org/base/head/include/mqueue.h?revision=306588&view=markup#l54)
//!   which lets one get a file descriptor from a `mqd_t`.  
//!   Changing or querying close-on-exec requires `AsRawFd`. `is_cloexec()` is
//!   always present and returns `true` on OSes where cloexec cannot be
//!   disabled. (posix message queue descriptors should have close-on-exec set
//!   by default).
//! * mio `Source`: The impl requires both `AsRawFd` and that mio compiless.
//!   This does not guarantee that the event notification mechanism used by mio
//!   supports posix message queues though.
//!
//! On Linux, message queues and their permissions can be viewed in
//! `/dev/mqueue/`. The kernel *can* be compiled to not support posix message
//! queues, so it's not guaranteed to always work. (such as on Android)
//!
//! On FreeBSD, the kernel module responsible for posix message queues
//! is not loaded by default; Run `kldload mqueuefs` as root to enable it.
//! To list queues, the file system must additionally be mounted first:
//! `mount -t mqueuefs null $somewhere`.
//! Versions before 11 do not have the function used to get a file descriptor,
//! so this library will not compile there.
//!
//! On NetBSD, re-opening message queues multiple times can eventually make all
//! further opens fail. This does not affect programs that open a single
//! queue once.  
//! The mio integration compiles, but registering message queues with mio fails.  
//! Because NetBSD ignores cloexec when opening or cloning descriptors, there
//! is a race condition with other threads exec'ing before this library can
//! enable close-on-exec for the descriptor.
//!
//! DragonFly doesn't set cloexec when opening either, but does when cloning.
//!
//! ## OS-dependent restrictions and default values
//!
//! Not even limiting oneself to the core features is enough to guarantee
//! portability!
//!
//! &nbsp; | Linux | FreeBSD | NetBSD | DragonFly | Illumos
//! -|-|-|-|-|-
//! max priority | 32767 | 63 | **31** | 31 | 31
//! default capacity | 10 | 10 | 32 | 32 | 128
//! default max_msg_len | 8192 | 1024 | 992 | 992 | 1024
//! max capacity | **10**\* | 100 | 512 | 512 | No limit
//! max max_msg_len | **8192**\* | 16384 | 16384 | 16384 | No limit
//! allows empty messages | Yes | Yes | No | No | Yes
//! enforces name rules | Yes | Yes | No | No | Yes
//! allows "/.", "/.." and "/" | No | No | Yes | Yes | Yes
//!
//! On Linux the listed size limits only apply to unprivileged processes.
//! As root there instead appears to be a combined limit on memory usage of the
//! form `capacity*(max_msg_len+k)`, but is several times higher than 10*8192.
//!
//! # Differences from the C API
//!
//! * `send()` and `receive()` tries again when EINTR / `ErrorKind::Interrupted`
//!   is returned. (Consistent with normal Rust io)
//! * `open()` and all other methods which take `AsRef<[u8]>` prepends '/' to
//!   the name if missing. (They allocate anyway, to append a terminating '\0')
//!
//! # Minimum Rust version
//!
//! The minimum supported Rust version is 1.31.
//!
//! # Possible additional features that haven't been implemented yet
//!
//! * Listing queues and their owners using OS-specific interfaces
//!   (such as /dev/mqueue/ on Linux)
//! * Querying and possibly changing limits and default values
//! * Struct that deletes the message queue when dropped
//! * tmpfile equivalent
//! * Test more platforms on CI
//! * `mq_notify()`?
//!
//! Please open an issue if you want any of them.

// # Why this crate requires `std`
//
// The libc crate doesn't expose `errno` in a portable way,
// so `std::io::Error::last_err()` is required to give errors
// more specific than "something went wrong".
// Depending on std also means that functions can use `io::Error` and
// `SystemTime` instead of custom types.

#![allow(clippy::needless_return, clippy::redundant_closure, clippy::needless_lifetimes)] // style
#![allow(clippy::cast_lossless)] // improves portability when values are limited by the OS anyway
// feel free to disable more lints

use std::{io, mem, ptr};
use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::io::ErrorKind;
use std::fmt::{self, Debug, Formatter};
#[cfg(not(any(target_os="illumos", target_os="solaris")))]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(not(any(target_os="freebsd", target_os="illumos", target_os="solaris")))]
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::time::{Duration, SystemTime};

extern crate libc;
use libc::{c_int, c_uint, c_char};
#[cfg(not(all(target_arch="x86_64", target_os="linux", target_pointer_width="32")))]
use libc::c_long;
use libc::{mqd_t, mq_open, mq_close, mq_unlink, mq_send, mq_receive};
use libc::{mq_attr, mq_getattr, mq_setattr};
use libc::{timespec, time_t, mq_timedsend, mq_timedreceive};
#[cfg(target_os="freebsd")]
use libc::mq_getfd_np;
use libc::{mode_t, O_ACCMODE, O_RDONLY, O_WRONLY, O_RDWR, O_CREAT, O_EXCL, O_NONBLOCK};
#[cfg(not(any(target_os="illumos", target_os="solaris")))]
use libc::{fcntl, ioctl, F_GETFD, FD_CLOEXEC, FIOCLEX, FIONCLEX};
#[cfg(not(any(target_os="freebsd", target_os="illumos", target_os="solaris")))]
use libc::F_DUPFD_CLOEXEC;

#[cfg(feature="mio_06")]
extern crate mio_06;
#[cfg(feature="mio_06")]
use mio_06::{event::Evented, unix::EventedFd, Ready, Poll, PollOpt};
#[cfg(feature="mio_07")]
extern crate mio_07;
#[cfg(feature="mio_07")]
use mio_07::{event::Source, unix::SourceFd, Registry, Interest};


/// Helper function for converting a `str` or byte slice into a C string
/// without allocating when possible.
///
/// The input is returned as-is if it starts with a '/' and ends with a '\0'.
/// Otherwise a new string is created with those characters included.
///
/// The name must not contain interior '\0' bytes.
///
/// # Panics
///
/// If the name contains interior NUL ('\0') bytes it is likely due to a bug,
/// so this function will then panic instead of returning a `Result`.
pub fn name_from_bytes<N: AsRef<[u8]> + ?Sized>(name: &N) -> Cow<CStr> {
    let name = name.as_ref();
    if !name.is_empty()  &&  name[0] == b'/'  &&  name[name.len()-1] == b'\0' {
        if let Ok(borrowed) = CStr::from_bytes_with_nul(name) {
            return Cow::Borrowed(borrowed);
        }
    } else {
        let mut owned = Vec::with_capacity(name.len()+2);
        if name.first() != Some(&b'/') {
            owned.push(b'/');
        }
        owned.extend_from_slice(name);
        if name.last() == Some(&b'\0') {
            owned.pop();
        }
        if let Ok(owned) = CString::new(owned) {
            return Cow::Owned(owned);
        }
    }
    panic!("Queue name contains interior '\0' bytes");
}

/// Internal helper for converting to C string and prepending '/' when missing.
fn name_to_cstring(name: &[u8]) -> Result<CString, io::Error> {
    let mut buf = Vec::with_capacity(name.len()+2);
    if name.first() != Some(&b'/') {
        buf.push(b'/');
    }
    buf.extend_from_slice(name);
    CString::new(buf).map_err(|err| io::Error::from(err) )
}

const CSTR_BUF_SIZE: usize = 32;
fn with_name_as_cstr<F: FnOnce(&CStr)->Result<R,io::Error>, R>(mut name: &[u8],  f: F)
-> Result<R,io::Error> {
    if name.first() == Some(&b'/') {
        name = &name[1..];
    }
    if name.len() > CSTR_BUF_SIZE - 2 {
        name_to_cstring(name).and_then(|longbuf| f(&longbuf) )
    } else {
        let mut shortbuf = [0; CSTR_BUF_SIZE];
        shortbuf[0] = b'/';
        shortbuf[1..name.len()+1].copy_from_slice(name);
        match CStr::from_bytes_with_nul(&shortbuf[..name.len()+2]) {
            Ok(name) => f(name),
            Err(_) => Err(io::Error::new(ErrorKind::InvalidInput, "contains nul byte"))
        }
    }
}


// Cannot use std::fs's because it doesn't expose getters,
// and rolling our own means we can also use it for mq-specific capacities.
/// Flags and parameters which control how a [`PosixMq`](struct.PosixMq.html)
/// message queue is opened or created.
#[derive(Clone,Copy, PartialEq,Eq)]
pub struct OpenOptions {
    flags: c_int,
    mode: mode_t,
    capacity: usize,
    max_msg_len: usize,
}

impl Debug for OpenOptions {
    fn fmt(&self,  fmtr: &mut Formatter) -> fmt::Result {
        fmtr.debug_struct("OpenOptions")
            .field(
                "read",
                &((self.flags & O_ACCMODE) == O_RDWR  ||  (self.flags & O_ACCMODE) == O_RDONLY)
             )
            .field(
                "write",
                &((self.flags & O_ACCMODE) == O_RDWR  ||  (self.flags & O_ACCMODE) == O_WRONLY)
             )
            .field("create", &(self.flags & O_CREAT != 0))
            .field("open", &(self.flags & O_EXCL == 0))
            .field("mode", &format_args!("{:03o}", self.mode))
            .field("capacity", &self.capacity)
            .field("max_msg_len", &self.max_msg_len)
            .field("nonblocking", &((self.flags & O_NONBLOCK) != 0))
            .finish()
    }
}

impl OpenOptions {
    fn new(flags: c_int) -> Self {
        OpenOptions {
            flags,
            // default permissions to only accessible for owner
            mode: 0o600,
            capacity: 0,
            max_msg_len: 0,
        }
    }

    /// Open message queue for receiving only.
    pub fn readonly() -> Self {
        OpenOptions::new(O_RDONLY)
    }

    /// Open message queue for sending only.
    pub fn writeonly() -> Self {
        OpenOptions::new(O_WRONLY)
    }

    /// Open message queue both for sending and receiving.
    pub fn readwrite() -> Self {
        OpenOptions::new(O_RDWR)
    }

    /// Set permissions to create the queue with.
    ///
    /// Some bits might be cleared by the process's umask when creating the
    /// queue, and unknown bits are ignored.
    ///
    /// This field is ignored if the queue already exists or should not be created.
    /// If this method is not called, queues are created with mode 600.
    pub fn mode(&mut self,  mode: u32) -> &mut Self {
        // 32bit value for consistency with std::os::unix even though only 12
        // bits are needed. Truncate if necessary because the OS ignores
        // unknown bits anyway. (and they're probably always zero as well).
        self.mode = mode as mode_t;
        return self;
    }

    /// Set the maximum size of each message.
    ///
    /// `receive()` will fail if given a buffer smaller than this value.
    ///
    /// If max_msg_len and capacity are both zero (or not set), the queue
    /// will be created with a maximum length and capacity decided by the
    /// operating system.  
    /// If this value is specified, capacity should also be, or opening the
    /// message queue might fail.
    pub fn max_msg_len(&mut self,  max_msg_len: usize) -> &mut Self {
        self.max_msg_len = max_msg_len;
        return self;
    }

    /// Set the maximum number of messages in the queue.
    ///
    /// When the queue is full, further `send()`s will either block
    /// or fail with an error of type `ErrorKind::WouldBlock`.
    ///
    /// If both capacity and max_msg_len are zero (or not set), the queue
    /// will be created with a maximum length and capacity decided by the
    /// operating system.  
    /// If this value is specified, max_msg_len should also be, or opening the
    /// message queue might fail.
    pub fn capacity(&mut self,  capacity: usize) -> &mut Self {
        self.capacity = capacity;
        return self;
    }

    /// Create message queue if it doesn't exist.
    pub fn create(&mut self) -> &mut Self {
        self.flags |= O_CREAT;
        self.flags &= !O_EXCL;
        return self;
    }

    /// Create a new queue, failing if the queue already exists.
    pub fn create_new(&mut self) -> &mut Self {
        self.flags |= O_CREAT | O_EXCL;
        return self;
    }

    /// Require the queue to already exist, failing if it doesn't.
    pub fn existing(&mut self) -> &mut Self {
        self.flags &= !(O_CREAT | O_EXCL);
        return self;
    }

    /// Open the message queue in non-blocking mode.
    ///
    /// This must be done if you want to use the message queue with mio.
    pub fn nonblocking(&mut self) -> &mut Self {
        self.flags |= O_NONBLOCK;
        return self;
    }

    /// Open a queue with the specified options.
    ///
    /// If the name doesn't start with a '/', one will be prepended.
    ///
    /// # Errors
    ///
    /// * Queue doesn't exist (ENOENT) => `ErrorKind::NotFound`
    /// * Name is just "/" (ENOENT) or is empty => `ErrorKind::NotFound`
    /// * Queue already exists (EEXISTS) => `ErrorKind::AlreadyExists`
    /// * Not permitted to open in this mode (EACCESS) => `ErrorKind::PermissionDenied`
    /// * More than one '/' in name (EACCESS) => `ErrorKind::PermissionDenied`
    /// * Invalid capacities (EINVAL) => `ErrorKind::InvalidInput`
    /// * Capacities too high (EMFILE) => `ErrorKind::Other`
    /// * Posix message queues are disabled (ENOSYS) => `ErrorKind::Other`
    /// * Name contains '\0' => `ErrorKind::InvalidInput`
    /// * Name is too long (ENAMETOOLONG) => `ErrorKind::Other`
    /// * Unlikely (ENFILE, EMFILE, ENOMEM, ENOSPC) => `ErrorKind::Other`
    /// * Possibly other
    pub fn open<N: AsRef<[u8]> + ?Sized>(&self,  name: &N) -> Result<PosixMq, io::Error> {
        pub fn open_slice(opts: &OpenOptions,  name: &[u8]) -> Result<PosixMq, io::Error> {
            with_name_as_cstr(name.as_ref(), |name| opts.open_c(&name) )
        }
        open_slice(self, name.as_ref())
    }

    /// Open a queue with the specified options and without inspecting `name`
    /// or allocating.
    ///
    /// This can on NetBSD be used to access message queues with names that
    /// doesn't start with a '/'.
    ///
    /// # Errors
    ///
    /// * Queue doesn't exist (ENOENT) => `ErrorKind::NotFound`
    /// * Name is just "/" (ENOENT) => `ErrorKind::NotFound`
    /// * Queue already exists (EEXISTS) => `ErrorKind::AlreadyExists`
    /// * Not permitted to open in this mode (EACCESS) => `ErrorKind::PermissionDenied`
    /// * More than one '/' in name (EACCESS) => `ErrorKind::PermissionDenied`
    /// * Invalid capacities (EINVAL) => `ErrorKind::InvalidInput`
    /// * Posix message queues are disabled (ENOSYS) => `ErrorKind::Other`
    /// * Name is empty (EINVAL) => `ErrorKind::InvalidInput`
    /// * Name is too long (ENAMETOOLONG) => `ErrorKind::Other`
    /// * Unlikely (ENFILE, EMFILE, ENOMEM, ENOSPC) => `ErrorKind::Other`
    /// * Possibly other
    pub fn open_c(&self,  name: &CStr) -> Result<PosixMq, io::Error> {
        let opts = self;

        // because mq_open is a vararg function, mode_t cannot be passed
        // directly on FreeBSD where it's smaller than c_int.
        let permissions = opts.mode as c_int;

        let mut capacities = unsafe { mem::zeroed::<mq_attr>() };
        let capacities_ptr = if opts.capacity != 0  ||  opts.max_msg_len != 0 {
            capacities.mq_maxmsg = opts.capacity as KernelLong;
            capacities.mq_msgsize = opts.max_msg_len as KernelLong;
            &mut capacities as *mut mq_attr
        } else {
            ptr::null_mut::<mq_attr>()
        };

        let mqd = unsafe { mq_open(name.as_ptr(), opts.flags, permissions, capacities_ptr) };
        // even when mqd_t is a pointer, -1 is the return value for error
        if mqd == -1isize as mqd_t {
            return Err(io::Error::last_os_error());
        }
        let mq = PosixMq{mqd};

        // NetBSD and DragonFly doesn't set cloexec by default and
        // ignores O_CLOEXEC. Setting it with FIOCLEX works though.
        // Propagate error if setting cloexec somehow fails, even though
        // close-on-exec won't matter in most cases.
        #[cfg(any(target_os="netbsd", target_os="dragonfly"))]
        mq.set_cloexec(true)?;

        Ok(mq)
    }
}


/// Delete a posix message queue.
///
/// # Errors
///
/// * Queue doesn't exist (ENOENT) => `ErrorKind::NotFound`
/// * Name is invalid (ENOENT or EACCESS) => `ErrorKind::NotFound` or `ErrorKind::PermissionDenied`
/// * Not permitted to delete the queue (EACCES) => `ErrorKind::PermissionDenied`
/// * Posix message queues are disabled (ENOSYS) => `ErrorKind::Other`
/// * Name contains '\0' bytes => `ErrorKind::InvalidInput`
/// * Name is too long (ENAMETOOLONG) => `ErrorKind::Other`
/// * Possibly other
pub fn unlink<N: AsRef<[u8]> + ?Sized>(name: &N) -> Result<(), io::Error> {
    fn unlink_slice(name: &[u8]) -> Result<(), io::Error> {
        with_name_as_cstr(name, |name| unlink_c(&name) )
    }
    unlink_slice(name.as_ref())
}

/// Delete a posix message queue, without inspecting `name` or allocating.
///
/// This can on NetBSD be used to access message queues with names that
/// doesn't start with a '/'.
///
/// # Errors
///
/// * Queue doesn't exist (ENOENT) => `ErrorKind::NotFound`
/// * Not permitted to delete the queue (EACCES) => `ErrorKind::PermissionDenied`
/// * Posix message queues are disabled (ENOSYS) => `ErrorKind::Other`
/// * More than one '/' in name (EACCESS) => `ErrorKind::PermissionDenied`
/// * Name is empty (EINVAL) => `ErrorKind::InvalidInput`
/// * Name is invalid (ENOENT, EACCESS or EINVAL) => `ErrorKind::NotFound`,
///   `ErrorKind::PermissionDenied` or `ErrorKind::InvalidInput`
/// * Name is too long (ENAMETOOLONG) => `ErrorKind::Other`
/// * Possibly other
pub fn unlink_c(name: &CStr) -> Result<(), io::Error> {
    let name = name.as_ptr();
    let ret = unsafe { mq_unlink(name) };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}


// The fields of `mq_attr` and `timespec` are of type `long` on all targets
// except x86_64-unknown-linux-gnux32, where they are `long long` (to match up
// with normal x86_64 `long`).
// Rusts lack of implicit widening makes this peculiarity annoying.
#[cfg(all(target_arch="x86_64", target_os="linux", target_pointer_width="32"))]
type KernelLong = i64;
#[cfg(not(all(target_arch="x86_64", target_os="linux", target_pointer_width="32")))]
type KernelLong = c_long;

/// Contains information about the capacities and state of a posix message queue.
///
/// Created by [`PosixMq::attributes()`](struct.PosixMq.html#method.attributes).
#[derive(Clone,Copy, PartialEq,Eq, Debug)]
pub struct Attributes {
    /// The maximum size of messages that can be stored in the queue.
    pub max_msg_len: usize,
    /// The maximum number of messages in the queue.
    pub capacity: usize,
    /// The number of messages currently in the queue at the time the
    /// attributes were retrieved.
    pub current_messages: usize,
    /// Whether the descriptor was set to nonblocking mode when
    /// the attributes were retrieved.
    pub nonblocking: bool,
}


macro_rules! retry_if_interrupted {($call:expr) => {{
    loop {// catch EINTR and retry
        let ret = $call;
        if ret != -1 {
            break ret;
        }
        let err = io::Error::last_os_error();
        if err.kind() != ErrorKind::Interrupted {
            return Err(err)
        }
    }
}}}

/// Returns saturated timespec as err if systemtime cannot be represented
fn deadline_to_realtime(deadline: SystemTime) -> Result<timespec, timespec> {
    /// Don't use struct literal in case timespec has extra fields on some platform.
    fn new_timespec(secs: time_t,  nsecs: KernelLong) -> timespec {
        let mut ts: timespec = unsafe { mem::zeroed() };
        ts.tv_sec = secs;
        ts.tv_nsec = nsecs;
        return ts;
    }

    // mq_timedsend() and mq_timedreceive() takes an absolute point in time,
    // based on CLOCK_REALTIME aka SystemTime.
    match deadline.duration_since(SystemTime::UNIX_EPOCH) {
        // Currently SystemTime has the same range as the C types, but
        // avoid truncation in case this changes.
        Ok(expires) if expires.as_secs() > time_t::max_value() as u64
            => Err(new_timespec(time_t::max_value(), 0)),
        Ok(expires)
            => Ok(new_timespec(expires.as_secs() as time_t, expires.subsec_nanos() as KernelLong)),
        // A pre-1970 deadline is probably a bug, but handle it anyway.
        // Based on https://github.com/solemnwarning/timespec/blob/master/README.md
        // the subsecond part of timespec should be positive and counts toward
        // positive infinity; (-1, 0) < (-1, 999999999) < (0, 0). This has the
        // advantage of simplifying addition and subtraction, but is the
        // opposite of Duration which counts away from zero.
        // The minimum representable value is therefore (-min_value(), 0)
        Err(ref earlier) if earlier.duration() > Duration::new(time_t::max_value() as u64 + 1, 0)
            => Err(new_timespec(time_t::min_value()+1, 0)), // add one to avoid negation bugs
        Err(ref earlier) if earlier.duration().subsec_nanos() == 0
            => Ok(new_timespec(-(earlier.duration().as_secs() as time_t), 0)),
        Err(earlier) => {
            // convert fractional part from counting away from zero to counting
            // toward positive infinity
            let before = earlier.duration();
            let secs = -(before.as_secs() as time_t) - 1;
            let nsecs = 1_000_000_000 - before.subsec_nanos() as KernelLong;
            Ok(new_timespec(secs, nsecs))
        }
    }
}

/// Returns an error if timeout is not representable or the produced deadline
/// overflows.
fn timeout_to_realtime(timeout: Duration) -> Result<timespec, io::Error> {
    if let Ok(now) = deadline_to_realtime(SystemTime::now()) {
        let mut expires = now;
        expires.tv_sec = expires.tv_sec.wrapping_add(timeout.as_secs() as time_t);
        // nanosecond values only use 30 bits, so adding two together is safe
        // even if tv_nsec is an i32
        expires.tv_nsec += timeout.subsec_nanos() as KernelLong;
        const NANO: KernelLong = 1_000_000_000;
        expires.tv_sec = expires.tv_sec.wrapping_add(expires.tv_nsec / NANO);
        expires.tv_nsec %= NANO;
        // check that the unsigned timeout is representable as a signed and
        // possibly smaller time_t, and the additions didn't overflow.
        // The second check will fail to catch Duration::new(!0, 999_999_999)
        // (which makes tv_sec wrap completely to the original value), but
        // the unsigned max value is not representable as a signed value and
        // will be caught by the first check.
        // Using wrapping_add and catching overflow afterwards avoids repeating
        // the error creation and also handles negative system time.
        if timeout.as_secs() > time_t::max_value() as u64  ||  expires.tv_sec < now.tv_sec {
            Err(io::Error::new(ErrorKind::InvalidInput, "timeout is too long"))
        } else {
            Ok(expires)
        }
    } else {
        Err(io::Error::new(ErrorKind::Other, "system time is not representable"))
    }
}


/// A descriptor for an open posix message queue.
///
/// Message queues can sent to and / or received from depending on the options
/// it was opened with.
///
/// The descriptor is closed when this struct is dropped.
pub struct PosixMq {
    mqd: mqd_t
}

impl PosixMq {
    /// Open an existing message queue in read-write mode.
    ///
    /// See [`OpenOptions::open()`](struct.OpenOptions.html#method.open) for
    /// details and possible errors.
    pub fn open<N: AsRef<[u8]> + ?Sized>(name: &N) -> Result<Self, io::Error> {
        OpenOptions::readwrite().open(name)
    }

    /// Open a message queue in read-write mode, creating it if it doesn't exists.
    ///
    /// See [`OpenOptions::open()`](struct.OpenOptions.html#method.open) for
    /// details and possible errors.
    pub fn create<N: AsRef<[u8]> + ?Sized>(name: &N) -> Result<Self, io::Error> {
        OpenOptions::readwrite().create().open(name)
    }


    /// Add a message to the queue.
    ///
    /// For maximum portability, avoid using priorities >= 32 or sending
    /// zero-length messages.
    ///
    /// # Errors
    ///
    /// * Queue is full and opened in nonblocking mode (EAGAIN) => `ErrorKind::WouldBlock`
    /// * Message is too big for the queue (EMSGSIZE) => `ErrorKind::Other`
    /// * Message is zero-length and the OS doesn't allow this (EMSGSIZE) => `ErrorKind::Other`
    /// * Priority is too high (EINVAL) => `ErrorKind::InvalidInput`
    /// * Queue is opened in read-only mode (EBADF) => `ErrorKind::Other`
    /// * Possibly other => `ErrorKind::Other`
    pub fn send(&self,  priority: u32,  msg: &[u8]) -> Result<(), io::Error> {
        let mptr = msg.as_ptr() as *const c_char;
        retry_if_interrupted!(unsafe { mq_send(self.mqd, mptr, msg.len(), priority as c_uint) });
        Ok(())
    }

    /// Take the message with the highest priority from the queue.
    ///
    /// The buffer must be at least as big as the maximum message length.
    ///
    /// # Errors
    ///
    /// * Queue is empty and opened in nonblocking mode (EAGAIN) => `ErrorKind::WouldBlock`
    /// * The receive buffer is smaller than the queue's maximum message size (EMSGSIZE) => `ErrorKind::Other`
    /// * Queue is opened in write-only mode (EBADF) => `ErrorKind::Other`
    /// * Possibly other => `ErrorKind::Other`
    pub fn receive(&self,  msgbuf: &mut [u8]) -> Result<(u32, usize), io::Error> {
        let bptr = msgbuf.as_mut_ptr() as *mut c_char;
        let mut priority = 0 as c_uint;
        let len = retry_if_interrupted!(
            unsafe { mq_receive(self.mqd, bptr, msgbuf.len(), &mut priority) }
        );
        // c_uint is unlikely to differ from u32, but even if it's bigger, the
        // range of supported values will likely be far smaller.
        Ok((priority as u32, len as usize))
    }

    /// Returns an `Iterator` which calls `receive()` repeatedly with an
    /// appropriately sized buffer.
    ///
    /// If the message queue is opened in non-blocking mode the iterator can be
    /// used to drain the queue. Otherwise it will block and never end.
    pub fn iter<'a>(&'a self) -> Iter<'a> {
        self.into_iter()
    }


    fn timedsend(&self,  priority: u32,  msg: &[u8],  deadline: &timespec)
    -> Result<(), io::Error> {
        let mptr = msg.as_ptr() as *const c_char;
        retry_if_interrupted!(unsafe {
            mq_timedsend(self.mqd, mptr, msg.len(), priority as c_uint, deadline)
        });
        Ok(())
    }

    /// Add a message to the queue or cancel if it's still full after a given
    /// duration.
    ///
    /// Returns immediately if opened in nonblocking mode, and the timeout has
    /// no effect.
    ///
    /// For maximum portability, avoid using priorities >= 32 or sending
    /// zero-length messages.
    ///
    /// # Errors
    ///
    /// * Timeout expired (ETIMEDOUT) => `ErrorKind::TimedOut`
    /// * Message is too big for the queue (EMSGSIZE) => `ErrorKind::Other`
    /// * OS doesn't allow empty messages (EMSGSIZE) => `ErrorKind::Other`
    /// * Priority is too high (EINVAL) => `ErrorKind::InvalidInput`
    /// * Queue is full and opened in nonblocking mode (EAGAIN) => `ErrorKind::WouldBlock`
    /// * Queue is opened in write-only mode (EBADF) => `ErrorKind::Other`
    /// * Timeout is too long / not representable => `ErrorKind::InvalidInput`
    /// * Possibly other => `ErrorKind::Other`
    pub fn send_timeout(&self,  priority: u32,  msg: &[u8],  timeout: Duration)
    -> Result<(), io::Error> {
        timeout_to_realtime(timeout).and_then(|expires| self.timedsend(priority, msg, &expires) )
    }

    /// Add a message to the queue or cancel if the queue is still full at a
    /// certain point in time.
    ///
    /// Returns immediately if opened in nonblocking mode, and the timeout has
    /// no effect.  
    /// The deadline is a `SystemTime` because the queues are intended for
    /// inter-process commonication, and `Instant` might be process-specific.
    ///
    /// For maximum portability, avoid using priorities >= 32 or sending
    /// zero-length messages.
    ///
    /// # Errors
    ///
    /// * Deadline reached (ETIMEDOUT) => `ErrorKind::TimedOut`
    /// * Message is too big for the queue (EMSGSIZE) => `ErrorKind::Other`
    /// * OS doesn't allow empty messages (EMSGSIZE) => `ErrorKind::Other`
    /// * Priority is too high (EINVAL) => `ErrorKind::InvalidInput`
    /// * Queue is full and opened in nonblocking mode (EAGAIN) => `ErrorKind::WouldBlock`
    /// * Queue is opened in write-only mode (EBADF) => `ErrorKind::Other`
    /// * Possibly other => `ErrorKind::Other`
    pub fn send_deadline(&self,  priority: u32,  msg: &[u8],  deadline: SystemTime)
    -> Result<(), io::Error> {
        match deadline_to_realtime(deadline) {
            Ok(expires) => self.timedsend(priority, msg, &expires),
            Err(_) => Err(io::Error::new(ErrorKind::InvalidInput, "deadline is not representable"))
        }
    }

    fn timedreceive(&self,  msgbuf: &mut[u8],  deadline: &timespec)
    -> Result<(u32, usize), io::Error> {
        let bptr = msgbuf.as_mut_ptr() as *mut c_char;
        let mut priority: c_uint = 0;
        let len = retry_if_interrupted!(
            unsafe { mq_timedreceive(self.mqd, bptr, msgbuf.len(), &mut priority, deadline) }
        );
        Ok((priority as u32, len as usize))
    }

    /// Take the message with the highest priority from the queue or cancel if
    /// the queue still empty after a given duration.
    ///
    /// Returns immediately if opened in nonblocking mode, and the timeout has
    /// no effect.
    ///
    /// # Errors
    ///
    /// * Timeout expired (ETIMEDOUT) => `ErrorKind::TimedOut`
    /// * The receive buffer is smaller than the queue's maximum message size (EMSGSIZE) => `ErrorKind::Other`
    /// * Queue is empty and opened in nonblocking mode (EAGAIN) => `ErrorKind::WouldBlock`
    /// * Queue is opened in read-only mode (EBADF) => `ErrorKind::Other`
    /// * Timeout is too long / not representable => `ErrorKind::InvalidInput`
    /// * Possibly other => `ErrorKind::Other`
    pub fn receive_timeout(&self,  msgbuf: &mut[u8],  timeout: Duration)
    -> Result<(u32, usize), io::Error> {
        timeout_to_realtime(timeout).and_then(|expires| self.timedreceive(msgbuf, &expires) )
    }

    /// Take the message with the highest priority from the queue or cancel if
    /// the queue is still empty at a point in time.
    ///
    /// Returns immediately if opened in nonblocking mode, and the timeout has
    /// no effect.  
    /// The deadline is a `SystemTime` because the queues are intended for
    /// inter-process commonication, and `Instant` might be process-specific.
    ///
    /// # Errors
    ///
    /// * Deadline reached (ETIMEDOUT) => `ErrorKind::TimedOut`
    /// * The receive buffer is smaller than the queue's maximum message size (EMSGSIZE) => `ErrorKind::Other`
    /// * Queue is empty and opened in nonblocking mode (EAGAIN) => `ErrorKind::WouldBlock`
    /// * Queue is opened in read-only mode (EBADF) => `ErrorKind::Other`
    /// * Possibly other => `ErrorKind::Other`
    pub fn receive_deadline(&self,  msgbuf: &mut[u8],  deadline: SystemTime)
    -> Result<(u32, usize), io::Error> {
        match deadline_to_realtime(deadline) {
            Ok(expires) => self.timedreceive(msgbuf, &expires),
            Err(_) => Err(io::Error::new(ErrorKind::InvalidInput, "deadline is not representable"))
        }
    }


    /// Get information about the state of the message queue.
    ///
    /// # Errors
    ///
    /// Retrieving these attributes should only fail if the underlying
    /// descriptor has been closed or is not a message queue.
    /// In that case `max_msg_len`, `capacity` and `current_messages` will be
    /// zero and `nonblocking` is set to `true`.
    ///
    /// The rationale for swallowing these errors is that they're only caused
    /// by buggy code (incorrect usage of `from_raw_fd()` or similar),
    /// and not having to `.unwrap()` makes the function nicer to use.  
    /// Future `send()` and `receive()` will reveal the bug when they also fail.
    /// (Which also means they won't block.)
    pub fn attributes(&self) -> Attributes {
        let mut attrs: mq_attr = unsafe { mem::zeroed() };
        let ret = unsafe { mq_getattr(self.mqd, &mut attrs) };
        if ret == -1 {
            Attributes { max_msg_len: 0,  capacity: 0,  current_messages: 0,  nonblocking: true }
        } else {
            Attributes {
                max_msg_len: attrs.mq_msgsize as usize,
                capacity: attrs.mq_maxmsg as usize,
                current_messages: attrs.mq_curmsgs as usize,
                nonblocking: (attrs.mq_flags & (O_NONBLOCK as KernelLong)) != 0,
            }
        }
    }

    /// Check whether this descriptor is in nonblocking mode.
    ///
    /// # Errors
    ///
    /// Returns `true` if retrieving the flag fails,
    /// see [`attributes()`](struct.PosixMq#method.attributes) for rationale.
    pub fn is_nonblocking(&self) -> bool {
        self.attributes().nonblocking
    }

    /// Enable or disable nonblocking mode for this descriptor.
    ///
    /// This can also be set when opening the message queue,
    /// with [`OpenOptions::nonblocking()`](struct.OpenOptions.html#method.nonblocking).
    ///
    /// # Errors
    ///
    /// Setting nonblocking mode should only fail due to incorrect usage of
    /// `from_raw_fd()` or `as_raw_fd()`, see the documentation on
    /// [`attributes()`](struct.PosixMq.html#method.attributes) for details.
    pub fn set_nonblocking(&self,  nonblocking: bool) -> Result<(), io::Error> {
        let mut attrs: mq_attr = unsafe { mem::zeroed() };
        attrs.mq_flags = if nonblocking {O_NONBLOCK as KernelLong} else {0};
        let res = unsafe { mq_setattr(self.mqd, &attrs, ptr::null_mut()) };
        if res == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }


    /// Create a new descriptor for the same message queue.
    ///
    /// The new descriptor will have close-on-exec set.
    ///
    /// This function is not available on FreeBSD, Illumos or Solaris.
    #[cfg(not(any(target_os="freebsd", target_os="illumos", target_os="solaris")))]
    pub fn try_clone(&self) -> Result<Self, io::Error> {
        let mq = match unsafe { fcntl(self.mqd, F_DUPFD_CLOEXEC, 0) } {
            -1 => return Err(io::Error::last_os_error()),
            fd => PosixMq{mqd: fd},
        };
        // NetBSD (but not DragonFly) ignores the cloexec part of F_DUPFD_CLOEXEC
        #[cfg(target_os="netbsd")]
        mq.set_cloexec(true)?;
        Ok(mq)
    }


    /// Check whether this descriptor will be closed if the process `exec`s
    /// into another program.
    ///
    /// Posix message queues are closed on exec by default, but this can be
    /// changed with `set_cloexec()`. On operating systems where that function
    /// is not available, this function unconditionally returns `true`.
    ///
    /// # Errors
    ///
    /// Retrieving this flag should only fail if the message queue descriptor
    /// is already closed. In that case `true` is returned as it will
    /// not be open after `exec`ing either.
    pub fn is_cloexec(&self) -> bool {
        #[cfg(not(any(target_os="illumos", target_os="solaris")))]
        {
            let flags = unsafe { fcntl(self.as_raw_fd(), F_GETFD) };
            if flags != -1 {
                return (flags & FD_CLOEXEC) != 0;
            }
        }
        return true;
    }

    /// Change close-on-exec for this descriptor.
    ///
    /// It is on by default, so this method should only be called when one
    /// wants the descriptor to remain open afte `exec`ing.
    ///
    /// This function is not available on Illumos or Solaris.
    ///
    /// # Errors
    ///
    /// This function should only fail if the underlying file descriptor has
    /// been closed (due to incorrect usage of `from_raw_fd()` or similar),
    /// and not reused for something else yet.
    #[cfg(not(any(target_os="illumos", target_os="solaris")))]
    pub fn set_cloexec(&self,  cloexec: bool) -> Result<(), io::Error> {
        let op = if cloexec {FIOCLEX} else {FIONCLEX};
        match unsafe { ioctl(self.as_raw_fd(), op) } {
            // Don't hide the error here, because callers can ignore the
            // returned value if they want.
            -1 => Err(io::Error::last_os_error()),
            _ => Ok(())
        }
    }


    /// Create a `PosixMq` from an already opened message queue descriptor.
    ///
    /// This function should only be used for ffi or if calling `mq_open()`
    /// directly for some reason.  
    /// Use [`from_raw_fd()`](#method.from_raw_fd) instead if the surrounding
    /// code requires `mqd_t` to be a file descriptor.
    ///
    /// # Safety
    ///
    /// On some operating systems `mqd_t` is a pointer, so the safety of most
    /// other methods then depend on it being correct.
    pub unsafe fn from_raw_mqd(mqd: mqd_t) -> Self {
        PosixMq{mqd}
    }

    /// Get the raw message queue descriptor.
    ///
    /// This function should only be used for passing to ffi code or to access
    /// portable features not exposed by this wrapper (such as calling
    /// `mq_notify()` or handling EINTR / `ErrorKind::Interrupted` when sending
    /// or receiving).
    ///
    /// If you need a file descriptor, use `as_raw_fd()` instead for increased
    /// portability.
    /// ([`as_raw_fd()`](#method.as_raw_fd) can sometimes retrieve an
    /// underlying file descriptor even if `mqd_t` is not an `int`.)
    pub fn as_raw_mqd(&self) -> mqd_t {
        self.mqd
    }

    /// Convert this wrapper into the raw message queue descriptor without
    /// closing it.
    ///
    /// This function should only be used for ffi; If you need a file
    /// descriptor use [`into_raw_fd()`](#method.into_raw_fd) instead.
    pub fn into_raw_mqd(self) -> mqd_t {
        let mqd = self.mqd;
        mem::forget(self);
        return mqd;
    }
}

/// Get an underlying file descriptor for the message queue.
///
/// If you just need the raw `mqd_t`, use
/// [`as_raw_mqd()`](struct.PosixMq.html#method.as_raw_mqd)
/// instead for increased portability.
///
/// This impl is not available on Illumos and Solaris.
#[cfg(not(any(target_os="illumos", target_os="solaris")))]
impl AsRawFd for PosixMq {
    // On Linux, NetBSD and DragonFly, `mqd_t` is a plain file descriptor
    // and can trivially be convverted, but this is not guaranteed, nor the
    // case on FreeBSD, Illumos and Solaris.
    #[cfg(not(target_os="freebsd"))]
    fn as_raw_fd(&self) -> RawFd {
        self.mqd
    }

    // FreeBSD has mq_getfd_np() (where _np stands for non-portable)
    #[cfg(target_os="freebsd")]
    fn as_raw_fd(&self) -> RawFd {
        unsafe { mq_getfd_np(self.mqd) }
    }
}

/// Create a `PosixMq` wrapper from a raw file descriptor.
///
/// Note that the message queue will be closed when the returned `PosixMq` goes
/// out of scope / is dropped.
///
/// This impl is not available on FreeBSD, Illumos or Solaris; If you got a
/// `mqd_t` in a portable fashion (from FFI code or by calling `mq_open()`
/// yourself for some reason), use
/// [`from_raw_mqd()`](struct.PosixMq.html#method.from_raw_mqd) instead.
#[cfg(not(any(target_os="freebsd", target_os="illumos", target_os="solaris")))]
impl FromRawFd for PosixMq {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        PosixMq{mqd: fd}
    }
}

/// Convert the `PosixMq` into a raw file descriptor without closing the
/// message queue.
///
/// This impl is not available on FreeBSD, Illumos or Solaris. If you need to
/// transfer ownership to FFI code accepting a `mqd_t`, use
/// [`into_raw_mqd()`](struct.PosixMq.html#method.into_raw_mqd) instead.
#[cfg(not(any(target_os="freebsd", target_os="illumos", target_os="solaris")))]
impl IntoRawFd for PosixMq {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.mqd;
        mem::forget(self);
        return fd;
    }
}


impl IntoIterator for PosixMq {
    type Item = (u32, Vec<u8>);
    type IntoIter = IntoIter;
    fn into_iter(self) -> IntoIter {
        IntoIter {
            max_msg_len: self.attributes().max_msg_len,
            mq: self,
        }
    }
}

impl<'a> IntoIterator for &'a PosixMq {
    type Item = (u32, Vec<u8>);
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        Iter {
            max_msg_len: self.attributes().max_msg_len,
            mq: self,
        }
    }
}


impl Debug for PosixMq {
    // Only show "fd" on operating systems where mqd_t is known to contain one.
    #[cfg(any(
        target_os="linux", target_os="freebsd",
        target_os="netbsd", target_os="dragonfly",
    ))]
    fn fmt(&self,  fmtr: &mut Formatter) -> fmt::Result {
        write!(fmtr, "PosixMq{{ fd: {} }}", self.as_raw_fd())
    }

    #[cfg(not(any(
        target_os="linux", target_os="freebsd",
        target_os="netbsd", target_os="dragonfly",
    )))]
    fn fmt(&self,  fmtr: &mut Formatter) -> fmt::Result {
        write!(fmtr, "PosixMq{{}}")
    }
}

impl Drop for PosixMq {
    fn drop(&mut self) {
        unsafe { mq_close(self.mqd) };
    }
}

// On some platforms mqd_t is a pointer, so Send and Sync aren't
// auto-implemented there. While I don't feel certain enough to
// blanket-implement Sync, I can't see why an implementation would make it UB
// to move operations to another thread.
unsafe impl Send for PosixMq {}

// On FreeBSD, mqd_t is a `struct{int fd, struct sigev_node* node}*`,
// but the sigevent is only accessed by `mq_notify()`, so it's thread-safe
// as long as that function requires `&mut self` or isn't exposed.
//  src: https://svnweb.freebsd.org/base/head/lib/librt/mq.c?view=markup
// On Illumos, mqd_t points to a rather complex struct, but the functions use
// mutexes and semaphores, so I assume they're totally thread-safe.
//  src: https://github.com/illumos/illumos-gate/blob/master/usr/src/lib/libc/port/rt/mqueue.c
// Solaris I assume is equivalent to Illumos, because the Illumos code has
// barely been modified after the initial source code release.
// Linux, NetBSD and DragonFly gets Sync auto-implemented because
// mqd_t is an int.
#[cfg(any(target_os="freebsd", target_os="illumos", target_os="solaris"))]
unsafe impl Sync for PosixMq {}


/// Allow receiving event notifications through mio (version 0.6).
///
/// This impl requires the `mio_06` feature to be enabled:
///
/// ```toml
/// [dependencies]
/// posixmq = {version="0.2", features=["mio_06"]}
/// ```
///
/// Remember to open the queue in non-blocking mode. (with `OpenOptions.noblocking()`)
#[cfg(feature="mio_06")]
impl Evented for PosixMq {
    fn register(&self,  poll: &Poll,  token: mio_06::Token,  interest: Ready,  opts: PollOpt)
    -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(&self,  poll: &Poll,  token: mio_06::Token,  interest: Ready,  opts: PollOpt)
    -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self,  poll: &Poll) -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}


/// Allow receiving event notifications through mio (version 0.7).
///
/// This impl requires the `mio_07` feature to be enabled:
///
/// ```toml
/// [dependencies]
/// posixmq = {version="0.2", features=["mio_07"]}
/// ```
///
/// Remember to open the queue in non-blocking mode. (with `OpenOptions.noblocking()`)
#[cfg(feature="mio_07")]
impl Source for &PosixMq {
    fn register(&mut self,  registry: &Registry,  token: mio_07::Token,  interest: Interest)
    -> Result<(), io::Error> {
        SourceFd(&self.as_raw_fd()).register(registry, token, interest)
    }

    fn reregister(&mut self,  registry: &Registry,  token: mio_07::Token,  interest: Interest)
    -> Result<(), io::Error> {
        SourceFd(&self.as_raw_fd()).reregister(registry, token, interest)
    }

    fn deregister(&mut self,  registry: &Registry) -> Result<(), io::Error> {
        SourceFd(&self.as_raw_fd()).deregister(registry)
    }
}

#[cfg(feature="mio_07")]
impl Source for PosixMq {
    fn register(&mut self,  registry: &Registry,  token: mio_07::Token,  interest: Interest)
    -> Result<(), io::Error> {
        {&mut &*self}.register(registry, token, interest)
    }

    fn reregister(&mut self,  registry: &Registry,  token: mio_07::Token,  interest: Interest)
    -> Result<(), io::Error> {
        {&mut &*self}.reregister(registry, token, interest)
    }

    fn deregister(&mut self,  registry: &Registry) -> Result<(), io::Error> {
        {&mut &*self}.deregister(registry)
    }
}


/// An `Iterator` that [`receive()`](struct.PosixMq.html#method.receive)s
/// messages from a borrowed [`PosixMq`](struct.PosixMq.html).
///
/// Iteration ends when a `receive()` fails with a `WouldBlock` error, but is
/// infinite if the descriptor is opened in blocking mode.
///
/// # Panics
///
/// `next()` can panic if an error of type other than `WouldBlock` is produced.
pub struct Iter<'a> {
    mq: &'a PosixMq,
    /// Cached
    max_msg_len: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (u32, Vec<u8>);
    fn next(&mut self) -> Option<(u32, Vec<u8>)> {
        let mut buf = vec![0; self.max_msg_len];
        match self.mq.receive(&mut buf) {
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => None,
            Err(e) => panic!("Cannot receive from posix message queue: {}", e),
            Ok((priority, len)) => {
                buf.truncate(len);
                Some((priority, buf))
            }
        }
    }
}

/// An `Iterator` that [`receive()`](struct.PosixMq.html#method.receive)s
/// messages from an owned [`PosixMq`](struct.PosixMq.html).
///
/// Iteration ends when a `receive()` fails with a `WouldBlock` error, but is
/// infinite if the descriptor is opened in blocking mode.
///
/// # Panics
///
/// `next()` can panic if an error of type other than `WouldBlock` is produced.
pub struct IntoIter {
    mq: PosixMq,
    max_msg_len: usize,
}

impl Iterator for IntoIter {
    type Item = (u32, Vec<u8>);
    fn next(&mut self) -> Option<(u32, Vec<u8>)> {
        Iter{mq: &self.mq, max_msg_len: self.max_msg_len}.next()
    }
}
