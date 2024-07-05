/* posixmq 1.0.0 - Idiomatic rust library for using posix message queues
 * Copyright 2019, 2020 Torbjørn Birch Moltu
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
//! For a longer introduction, see [`man mq_overview`](http://man7.org/linux/man-pages/man7/mq_overview.7.html)
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
//! posixmq::remove_queue("/hello_posixmq").unwrap();
//!
//! // the receive buffer must be at least as big as the biggest possible
//! // message, or you will not be allowed to receive anything.
//! let mut buf = vec![0; mq.attributes().unwrap().max_msg_len];
//! assert_eq!(mq.recv(&mut buf).unwrap(), (10, "Hello,".len()));
//! assert_eq!(mq.recv(&mut buf).unwrap(), (0, "message".len()));
//! assert_eq!(mq.recv(&mut buf).unwrap(), (0, "queue".len()));
//! assert_eq!(&buf[..5], b"queue");
//!
//! // check that there are no more messages
//! assert_eq!(mq.attributes().unwrap().current_messages, 0);
//! // note that acting on this value is race-prone. A better way to do this
//! // would be to switch our descriptor to non-blocking mode, and check for
//! // an error of type `ErrorKind::WouldBlock`.
//! ```
//!
//! With mio (and `features = ["mio_07"]` in Cargo.toml):
#![cfg_attr(feature = "mio_07", doc = "```")]
#![cfg_attr(not(feature = "mio_07"), doc = "```compile_fail")]
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
//!     posixmq::remove_queue("/mio").unwrap();
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
//!             match receiver.recv(&mut buf) {
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
//! &nbsp; | Linux | FreeBSD 11+ | NetBSD | DragonFly BSD | Illumos | Solaris | VxWorks
//! -|-|-|-|-|-|-|-
//! core features | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes
//! mio `Source` & `Evented` | Yes | Yes | unusable | Yes | No | No | No
//! `FromRawFd`+`IntoRawFd`+[`try_clone()`](struct.PosixMq.html#method.try_clone) | Yes | No | Yes | Yes | No | No | No
//! `AsRawFd`+[`set_cloexec()`](struct.PosixMq.html#method.set_cloexec) | Yes | Yes | Yes | Yes | No | No | No
//! Tested? | Manually+CI | Manually+CI | Manually | Manually | Manually (on OmniOSce) | Cross-`check`ed on CI | No
//!
//! This library will fail to compile if the target OS doesn't have posix
//! message queues.
//!
//! Feature explanations:
//!
//! * `FromRawFd`+`IntoRawFd`+[`try_clone()`](struct.PosixMq.html#method.try_clone):
//!   For theese to work, the inner `mqd_t` type must be an `int`/`RawFd` typedef,
//!   and known to represent a file descriptor.
//!   These impls are only available on OSes where this is known to be the case,
//!   to increase the likelyhood that the core features will compile on an
//!   unknown OS.
//! * `AsRawFd`+[`set_cloexec()`](struct.PosixMq.html#method.set_cloexec):
//!   Similar to `FromRawFd` and `IntoRawFd`, but FreeBSD 11+ has [a function](https://svnweb.freebsd.org/base/head/include/mqueue.h?revision=306588&view=markup#l54)
//!   which lets one get a file descriptor from a `mqd_t`.
//!   Changing or querying close-on-exec requires `AsRawFd`, and is only
//!   only meaningful on operating systems that have the concept of `exec()`.
//!   [`is_cloexec()`](struct.PosixMq.html#method.is_cloexec) is always present
//!   and returns `true` on OSes where close-on-exec cannot be disabled or one
//!   cannot `exec()`. (posix message queue descriptors should have
//!   close-on-exec set by default).
//! * mio `Source` & `Evented`: The impls require both `AsRawFd`
//!   and that mio compiles on the OS.
//!   This does not guarantee that the event notification mechanism used by mio
//!   supports posix message queues though. (registering fails on NetBSD)
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
//! DragonFly BSD doesn't set cloexec when opening either, but does when
//! cloning.
//!
//! ## OS-dependent restrictions and default values
//!
//! Not even limiting oneself to the core features is enough to guarantee
//! portability!
//!
//! &nbsp; | Linux | FreeBSD | NetBSD | DragonFly BSD | Illumos
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
//! * [`send()`](struct.PosixMq.html#method.send),
//!   [`recv()`](struct.PosixMq.html#method.recv) and the timed equivalents
//!   tries again when EINTR / `ErrorKind::Interrupted` is returned.
//!   (Consistent with how std does IO)
//! * `open()` and all other methods which take `AsRef<[u8]>` prepends `'/'` to
//!   the name if missing.
//!   (They have to copy the name anyway, to append a terminating `'\0'`)
//!   Use [`open_c()`](struct.OpenOptions.html#method.open_c) and
//!   [`remove_queue_c()`](fn.remove_queue_c.html) if you need to interact with
//!   queues on NetBSD or DragonFly that doesn't have a leading `'/'`.
//!
//! # Minimum supported Rust version
//!
//! The minimum supported Rust version for posixmq 1.0.z releases is 1.31.1.
//! Later 1.y.0 releases might increase this. Until rustup has builds for
//! DragonFly BSD and Illumos, the minimum version will not be increased past
//! what is available in the repositories for those operating systems.

// # Why this crate requires `std`
//
// The libc crate doesn't expose `errno` in a portable way,
// so `std::io::Error::last_os_error()` is required to give errors
// more specific than "something went wrong".
// Depending on std also means that functions can use `io::Error` and
// `SystemTime` instead of custom types.

use std::ffi::CStr;
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "dragonfly",
))]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(any(target_os = "linux", target_os = "netbsd", target_os = "dragonfly"))]
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::time::{Duration, SystemTime};
use std::{fmt, io, mem, ptr};

#[cfg(not(all(
    target_arch = "x86_64",
    target_os = "linux",
    target_pointer_width = "32"
)))]
use libc::c_long;
#[cfg(target_os = "freebsd")]
use libc::mq_getfd_np;
#[cfg(any(target_os = "linux", target_os = "netbsd", target_os = "dragonfly"))]
use libc::F_DUPFD_CLOEXEC;
use libc::{c_char, c_int, c_uint};
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "dragonfly",
))]
use libc::{fcntl, ioctl, FD_CLOEXEC, FIOCLEX, FIONCLEX, F_GETFD};
use libc::{mode_t, O_ACCMODE, O_CREAT, O_EXCL, O_NONBLOCK, O_RDONLY, O_RDWR, O_WRONLY};
use libc::{mq_attr, mq_getattr, mq_setattr};
use libc::{mq_close, mq_open, mq_receive, mq_send, mq_unlink, mqd_t};
use libc::{mq_timedreceive, mq_timedsend, time_t, timespec};

#[cfg(any(
    feature = "mio_06",
    feature = "mio_07",
    feature = "mio_08",
    feature = "mio_1"
))]
mod mio;

const CSTR_BUF_SIZE: usize = 48;
fn with_name_as_cstr<F: FnOnce(&CStr) -> Result<R, io::Error>, R>(
    mut name: &[u8],
    f: F,
) -> Result<R, io::Error> {
    if name.first() == Some(&b'/') {
        name = &name[1..];
    }
    let mut longbuf: Box<[u8]>;
    let mut shortbuf: [u8; CSTR_BUF_SIZE];
    let c_bytes = if name.len() + 2 <= CSTR_BUF_SIZE {
        shortbuf = [0; CSTR_BUF_SIZE];
        &mut shortbuf[..name.len() + 2]
    } else {
        longbuf = vec![0; name.len() + 2].into_boxed_slice();
        &mut longbuf
    };
    c_bytes[0] = b'/';
    c_bytes[1..name.len() + 1].copy_from_slice(name);

    match CStr::from_bytes_with_nul(c_bytes) {
        Ok(name) => f(name),
        Err(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "contains nul byte",
        )),
    }
}

// Cannot use std::fs's because it doesn't expose getters,
// and rolling our own means we can also use it for mq-specific capacities.
/// Flags and parameters which control how a [`PosixMq`](struct.PosixMq.html)
/// message queue is opened or created.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct OpenOptions {
    flags: c_int,
    mode: mode_t,
    capacity: usize,
    max_msg_len: usize,
}

impl fmt::Debug for OpenOptions {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        fmtr.debug_struct("OpenOptions")
            .field(
                "read",
                &((self.flags & O_ACCMODE) == O_RDWR || (self.flags & O_ACCMODE) == O_RDONLY),
            )
            .field(
                "write",
                &((self.flags & O_ACCMODE) == O_RDWR || (self.flags & O_ACCMODE) == O_WRONLY),
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
    pub fn mode(&mut self, mode: u32) -> &mut Self {
        // 32bit value for consistency with std::os::unix even though only 12
        // bits are needed. Truncate if necessary because the OS ignores
        // unknown bits anyway. (and they're probably always zero as well).
        self.mode = mode as mode_t;
        self
    }

    /// Set the maximum size of each message.
    ///
    /// `recv()` will fail if given a buffer smaller than this value.
    ///
    /// If max_msg_len and capacity are both zero (or not set), the queue
    /// will be created with a maximum length and capacity decided by the
    /// operating system.
    /// If this value is specified, capacity should also be, or opening the
    /// message queue might fail.
    pub fn max_msg_len(&mut self, max_msg_len: usize) -> &mut Self {
        self.max_msg_len = max_msg_len;
        self
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
    pub fn capacity(&mut self, capacity: usize) -> &mut Self {
        self.capacity = capacity;
        self
    }

    /// Create message queue if it doesn't exist.
    pub fn create(&mut self) -> &mut Self {
        self.flags |= O_CREAT;
        self.flags &= !O_EXCL;
        self
    }

    /// Create a new queue, failing if the queue already exists.
    pub fn create_new(&mut self) -> &mut Self {
        self.flags |= O_CREAT | O_EXCL;
        self
    }

    /// Require the queue to already exist, failing if it doesn't.
    pub fn existing(&mut self) -> &mut Self {
        self.flags &= !(O_CREAT | O_EXCL);
        self
    }

    /// Open the message queue in non-blocking mode.
    ///
    /// This must be done if you want to use the message queue with mio.
    pub fn nonblocking(&mut self) -> &mut Self {
        self.flags |= O_NONBLOCK;
        self
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
    pub fn open<N: AsRef<[u8]> + ?Sized>(&self, name: &N) -> Result<PosixMq, io::Error> {
        pub fn open_slice(opts: &OpenOptions, name: &[u8]) -> Result<PosixMq, io::Error> {
            with_name_as_cstr(name, |name| opts.open_c(name))
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
    pub fn open_c(&self, name: &CStr) -> Result<PosixMq, io::Error> {
        let opts = self;

        // because mq_open is a vararg function, mode_t cannot be passed
        // directly on FreeBSD where it's smaller than c_int.
        let permissions = opts.mode as c_int;

        let mut capacities = unsafe { mem::zeroed::<mq_attr>() };
        let capacities_ptr = if opts.capacity != 0 || opts.max_msg_len != 0 {
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
        let mq = PosixMq { mqd };

        // NetBSD and DragonFly BSD doesn't set cloexec by default and
        // ignores O_CLOEXEC. Setting it with FIOCLEX works though.
        // Propagate error if setting cloexec somehow fails, even though
        // close-on-exec won't matter in most cases.
        #[cfg(any(target_os = "netbsd", target_os = "dragonfly"))]
        mq.set_cloexec(true)?;

        Ok(mq)
    }
}

/// Delete a posix message queue.
///
/// A `'/'` is prepended to the name if it doesn't start with one already.
/// (it would have to append a `'\0'` and therefore allocate or copy anyway.)
///
/// Processes that have it open will still be able to use it.
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
pub fn remove_queue<N: AsRef<[u8]> + ?Sized>(name: &N) -> Result<(), io::Error> {
    fn remove_queue_slice(name: &[u8]) -> Result<(), io::Error> {
        with_name_as_cstr(name, remove_queue_c)
    }
    remove_queue_slice(name.as_ref())
}

/// Delete a posix message queue, without inspecting `name` or allocating.
///
/// This function is on NetBSD necessary to remove queues with names that
/// doesn't start with a '/'.
///
/// # Errors
///
/// * Queue doesn't exist (ENOENT) => `ErrorKind::NotFound`
/// * Not permitted to delete the queue (EACCES) => `ErrorKind::PermissionDenied`
/// * Posix message queues are disabled (ENOSYS) => `ErrorKind::Other`
/// * More than one '/' in name (EACCESS) => `ErrorKind::PermissionDenied`
/// * Name is empty (EINVAL) => `ErrorKind::InvalidInput`
/// * Name is invalid (ENOENT, EACCESS or EINVAL) => `ErrorKind::NotFound`
///   `ErrorKind::PermissionDenied` or `ErrorKind::InvalidInput`
/// * Name is too long (ENAMETOOLONG) => `ErrorKind::Other`
/// * Possibly other
pub fn remove_queue_c(name: &CStr) -> Result<(), io::Error> {
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
#[cfg(all(
    target_arch = "x86_64",
    target_os = "linux",
    target_pointer_width = "32"
))]
type KernelLong = i64;
#[cfg(not(all(
    target_arch = "x86_64",
    target_os = "linux",
    target_pointer_width = "32"
)))]
type KernelLong = c_long;

/// Contains information about the capacities and state of a posix message queue.
///
/// Created by [`PosixMq::attributes()`](struct.PosixMq.html#method.attributes).
#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq, Default)]
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

impl fmt::Debug for Attributes {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        fmtr.debug_struct("Attributes")
            .field("max_msg_len", &self.max_msg_len)
            .field("capacity", &self.capacity)
            .field("current_messages", &self.current_messages)
            .field("nonblocking", &self.nonblocking)
            .finish()
    }
}

macro_rules! retry_if_interrupted {
    ($call:expr) => {{
        loop {
            // catch EINTR and retry
            let ret = $call;
            if ret != -1 {
                break ret;
            }
            let err = io::Error::last_os_error();
            if err.kind() != io::ErrorKind::Interrupted {
                return Err(err);
            }
        }
    }};
}

/// Returns saturated timespec as err if systemtime cannot be represented
#[allow(arithmetic_overflow)]
fn deadline_to_realtime(deadline: SystemTime) -> Result<timespec, timespec> {
    /// Don't use struct literal in case timespec has extra fields on some platform.
    fn new_timespec(secs: time_t, nsecs: KernelLong) -> timespec {
        let mut ts: timespec = unsafe { mem::zeroed() };
        ts.tv_sec = secs;
        ts.tv_nsec = nsecs;
        ts
    }

    // mq_timedsend() and mq_timedreceive() takes an absolute point in time,
    // based on CLOCK_REALTIME aka SystemTime.
    match deadline.duration_since(SystemTime::UNIX_EPOCH) {
        // Currently SystemTime has the same range as the C types, but
        // avoid truncation in case this changes.
        Ok(expires) if expires.as_secs() > time_t::MAX as u64 => Err(new_timespec(time_t::MAX, 0)),
        Ok(expires) => Ok(new_timespec(
            expires.as_secs() as time_t,
            expires.subsec_nanos() as KernelLong,
        )),
        // A pre-1970 deadline is probably a bug, but handle it anyway.
        // Based on https://github.com/solemnwarning/timespec/blob/master/README.md
        // the subsecond part of timespec should be positive and counts toward
        // positive infinity; (-1, 0) < (-1, 999999999) < (0, 0). This has the
        // advantage of simplifying addition and subtraction, but is the
        // opposite of Duration which counts away from zero.
        // The minimum representable value is therefore (-min_value(), 0)
        Err(ref earlier) if earlier.duration() > Duration::new(time_t::MAX as u64 + 1, 0) => {
            Err(new_timespec(time_t::MAX + 1, 0))
        } // add one to avoid negation bugs
        Err(ref earlier) if earlier.duration().subsec_nanos() == 0 => {
            Ok(new_timespec(-(earlier.duration().as_secs() as time_t), 0))
        }
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
        if timeout.as_secs() > time_t::MAX as u64 || expires.tv_sec < now.tv_sec {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "timeout is too long",
            ))
        } else {
            Ok(expires)
        }
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "system time is not representable",
        ))
    }
}

/// A descriptor for an open posix message queue.
///
/// Message queues can be sent to and / or received from depending on the
/// options it was opened with.
///
/// The descriptor is closed when this struct is dropped.
///
/// See [the documentation in the crate root](index.html) for examples,
/// portability notes and OS details.
pub struct PosixMq {
    mqd: mqd_t,
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
    pub fn send(&self, priority: u32, msg: &[u8]) -> Result<(), io::Error> {
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
    pub fn recv(&self, msgbuf: &mut [u8]) -> Result<(u32, usize), io::Error> {
        let bptr = msgbuf.as_mut_ptr() as *mut c_char;
        let mut priority = 0 as c_uint;
        let len = retry_if_interrupted!(unsafe {
            mq_receive(self.mqd, bptr, msgbuf.len(), &mut priority)
        });
        // c_uint is unlikely to differ from u32, but even if it's bigger, the
        // range of supported values will likely be far smaller.
        Ok((priority as u32, len as usize))
    }

    /// Returns an `Iterator` which calls [`recv()`](#method.recv) repeatedly
    /// with an appropriately sized buffer.
    ///
    /// If the message queue is opened in non-blocking mode the iterator can be
    /// used to drain the queue. Otherwise it will block and never end.
    pub fn iter(&self) -> Iter<'_> {
        self.into_iter()
    }

    fn timedsend(&self, priority: u32, msg: &[u8], deadline: &timespec) -> Result<(), io::Error> {
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
    pub fn send_timeout(
        &self,
        priority: u32,
        msg: &[u8],
        timeout: Duration,
    ) -> Result<(), io::Error> {
        timeout_to_realtime(timeout).and_then(|expires| self.timedsend(priority, msg, &expires))
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
    pub fn send_deadline(
        &self,
        priority: u32,
        msg: &[u8],
        deadline: SystemTime,
    ) -> Result<(), io::Error> {
        match deadline_to_realtime(deadline) {
            Ok(expires) => self.timedsend(priority, msg, &expires),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "deadline is not representable",
            )),
        }
    }

    fn timedreceive(
        &self,
        msgbuf: &mut [u8],
        deadline: &timespec,
    ) -> Result<(u32, usize), io::Error> {
        let bptr = msgbuf.as_mut_ptr() as *mut c_char;
        let mut priority: c_uint = 0;
        let len = retry_if_interrupted!(unsafe {
            mq_timedreceive(self.mqd, bptr, msgbuf.len(), &mut priority, deadline)
        });
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
    pub fn recv_timeout(
        &self,
        msgbuf: &mut [u8],
        timeout: Duration,
    ) -> Result<(u32, usize), io::Error> {
        timeout_to_realtime(timeout).and_then(|expires| self.timedreceive(msgbuf, &expires))
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
    pub fn recv_deadline(
        &self,
        msgbuf: &mut [u8],
        deadline: SystemTime,
    ) -> Result<(u32, usize), io::Error> {
        match deadline_to_realtime(deadline) {
            Ok(expires) => self.timedreceive(msgbuf, &expires),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "deadline is not representable",
            )),
        }
    }

    /// Get information about the state of the message queue.
    ///
    /// # Errors
    ///
    /// Retrieving these attributes should only fail if the underlying
    /// descriptor has been closed or is not a message queue.
    ///
    /// On operating systems where the descriptor is a pointer, such as on
    /// FreeBSD and Illumos, such bugs will enable undefined behavior
    /// and this call will dereference freed or uninitialized memory.
    /// (That doesn't make this function unsafe though -
    /// [`PosixMq::from_raw_mqd()`](#method.from_raw_mqd) and `mq_close()` are.)
    ///
    /// While a `send()` or `recv()` ran in place of this call would also have
    /// failed immediately and therefore not blocked, The descriptor might have
    /// become used for another queue when a *later* `send()` or `recv()` is
    /// performed. The descriptor might then be in blocking mode.
    ///
    /// # Examples
    ///
    /// ```
    /// # let _ = posixmq::remove_queue("/with_custom_capacity");
    /// let mq = posixmq::OpenOptions::readwrite()
    ///     .create_new()
    ///     .max_msg_len(100)
    ///     .capacity(3)
    ///     .open("/with_custom_capacity")
    ///     .expect("create queue");
    /// let attrs = mq.attributes().expect("get attributes for queue");
    /// assert_eq!(attrs.max_msg_len, 100);
    /// assert_eq!(attrs.capacity, 3);
    /// assert_eq!(attrs.current_messages, 0);
    /// assert!(!attrs.nonblocking);
    /// ```
    ///
    /// Ignore the error:
    ///
    /// (Will only happen with buggy code (incorrect usage of
    /// [`from_raw_fd()`](#method.from_raw_fd) or similar)).
    ///
    #[cfg_attr(
        any(
            target_os = "linux",
            target_os = "android",
            target_os = "netbsd",
            target_os = "dragonfly"
        ),
        doc = "```"
    )]
    #[cfg_attr(
        not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "netbsd",
            target_os = "dragonfly"
        )),
        doc = "```no_compile"
    )]
    /// # use std::os::unix::io::FromRawFd;
    /// # let bad = unsafe { posixmq::PosixMq::from_raw_fd(-1) };
    /// let attrs = bad.attributes().unwrap_or_default();
    /// assert_eq!(attrs.max_msg_len, 0);
    /// assert_eq!(attrs.capacity, 0);
    /// assert_eq!(attrs.current_messages, 0);
    /// assert!(!attrs.nonblocking);
    /// ```
    pub fn attributes(&self) -> Result<Attributes, io::Error> {
        let mut attrs: mq_attr = unsafe { mem::zeroed() };
        if unsafe { mq_getattr(self.mqd, &mut attrs) } == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Attributes {
                max_msg_len: attrs.mq_msgsize as usize,
                capacity: attrs.mq_maxmsg as usize,
                current_messages: attrs.mq_curmsgs as usize,
                nonblocking: (attrs.mq_flags & (O_NONBLOCK as KernelLong)) != 0,
            })
        }
    }

    /// Check whether this descriptor is in nonblocking mode.
    ///
    /// # Errors
    ///
    /// Should only fail as result of buggy code that either created this
    /// descriptor from something that is not a queue, or has already closed
    /// the underlying descriptor.
    /// (This function will not silently succeed if the fd points to anything
    /// other than a queue (for example a socket), as this function
    /// is a wrapper around [`attributes()`][#method.attributes].)
    /// To ignore failure, one can write `.is_nonblocking().unwrap_or(false)`.
    ///
    /// ## An error doesn't guarantee that any further [`send()`](#method.send) or [`recv()`](#method.recv) wont block.
    ///
    /// While a `send()` or `recv()` ran in place of this call would also have
    /// failed immediately and therefore not blocked, the descriptor might have
    /// become used for another queue when a *later* `send()` or `recv()` is
    /// performed. The descriptor might then be in blocking mode.
    pub fn is_nonblocking(&self) -> Result<bool, io::Error> {
        match self.attributes() {
            Ok(attrs) => Ok(attrs.nonblocking),
            Err(e) => Err(e),
        }
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
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), io::Error> {
        let mut attrs: mq_attr = unsafe { mem::zeroed() };
        attrs.mq_flags = if nonblocking {
            O_NONBLOCK as KernelLong
        } else {
            0
        };
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
    #[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "netbsd"))]
    pub fn try_clone(&self) -> Result<Self, io::Error> {
        let mq = match unsafe { fcntl(self.mqd, F_DUPFD_CLOEXEC, 0) } {
            -1 => return Err(io::Error::last_os_error()),
            fd => PosixMq { mqd: fd },
        };
        // NetBSD ignores the cloexec part of F_DUPFD_CLOEXEC
        // (but DragonFly BSD respects it here)
        #[cfg(target_os = "netbsd")]
        mq.set_cloexec(true)?;
        Ok(mq)
    }

    /// Check whether this descriptor will be closed if the process `exec`s
    /// into another program.
    ///
    /// Posix message queues are closed on exec by default,
    /// but this can be changed with [`set_cloexec()`](#method.set_cloexec).
    ///
    /// This function is not available on Illumos, Solaris or VxWorks.
    ///
    /// # Errors
    ///
    /// Retrieving this flag should only fail if the descriptor
    /// is already closed.
    /// In that case it will obviously not be open after execing,
    /// so treating errors as `true` should be safe.
    ///
    /// # Examples
    ///
    /// ```
    /// let queue = posixmq::PosixMq::create("is_cloexec").expect("open queue");
    /// # posixmq::remove_queue("is_cloexec").expect("delete queue");
    /// assert!(queue.is_cloexec().unwrap_or(true));
    /// ```
    pub fn is_cloexec(&self) -> Result<bool, io::Error> {
        #[cfg(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "dragonfly",
        ))]
        match unsafe { fcntl(self.as_raw_fd(), F_GETFD) } {
            -1 => Err(io::Error::last_os_error()),
            flags => Ok((flags & FD_CLOEXEC) != 0),
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "dragonfly",
        )))]
        Err(io::Error::new(
            io::ErrorKind::Other,
            "close-on-exec information is not available",
        ))
    }

    /// Change close-on-exec for this descriptor.
    ///
    /// It is on by default, so this method should only be called when one
    /// wants the descriptor to remain open afte `exec`ing.
    ///
    /// This function is not available on Illumos, Solaris or VxWorks.
    ///
    /// # Errors
    ///
    /// This function should only fail if the underlying file descriptor has
    /// been closed (due to incorrect usage of `from_raw_fd()` or similar),
    /// and not reused for something else yet.
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "dragonfly",
    ))]
    pub fn set_cloexec(&self, cloexec: bool) -> Result<(), io::Error> {
        let op = if cloexec { FIOCLEX } else { FIONCLEX };
        match unsafe { ioctl(self.as_raw_fd(), op) } {
            // Don't hide the error here, because callers can ignore the
            // returned value if they want.
            -1 => Err(io::Error::last_os_error()),
            _ => Ok(()),
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
    /// On some operating systems `mqd_t` is a pointer, which means that the
    /// safety of most other methods depend on it being correct.
    pub unsafe fn from_raw_mqd(mqd: mqd_t) -> Self {
        PosixMq { mqd }
    }

    /// Get the raw message queue descriptor.
    ///
    /// This function should only be used for passing to ffi code or to access
    /// portable features not exposed by this wrapper (such as calling
    /// `mq_notify()` or not automatically retrying on EINTR /
    /// `ErrorKind::Interrupted` when sending or receiving).
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
        mqd
    }
}

/// Get an underlying file descriptor for the message queue.
///
/// If you just need the raw `mqd_t`, use
/// [`as_raw_mqd()`](struct.PosixMq.html#method.as_raw_mqd)
/// instead for increased portability.
///
/// This impl is not available on Illumos, Solaris or VxWorks.
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "dragonfly",
))]
impl AsRawFd for PosixMq {
    // On Linux, NetBSD and DragonFly BSD, `mqd_t` is a plain file descriptor
    // and can trivially be convverted, but this is not guaranteed, nor the
    // case on FreeBSD, Illumos and Solaris.
    #[cfg(not(target_os = "freebsd"))]
    fn as_raw_fd(&self) -> RawFd {
        self.mqd
    }

    // FreeBSD has mq_getfd_np() (where _np stands for non-portable)
    #[cfg(target_os = "freebsd")]
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
#[cfg(any(target_os = "linux", target_os = "netbsd", target_os = "dragonfly"))]
impl FromRawFd for PosixMq {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        PosixMq { mqd: fd }
    }
}

/// Convert the `PosixMq` into a raw file descriptor without closing the
/// message queue.
///
/// This impl is not available on FreeBSD, Illumos or Solaris. If you need to
/// transfer ownership to FFI code accepting a `mqd_t`, use
/// [`into_raw_mqd()`](struct.PosixMq.html#method.into_raw_mqd) instead.
#[cfg(any(target_os = "linux", target_os = "netbsd", target_os = "dragonfly"))]
impl IntoRawFd for PosixMq {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.mqd;
        mem::forget(self);
        fd
    }
}

impl IntoIterator for PosixMq {
    type Item = (u32, Vec<u8>);
    type IntoIter = IntoIter;
    fn into_iter(self) -> IntoIter {
        IntoIter {
            max_msg_len: match self.attributes() {
                Ok(attrs) => attrs.max_msg_len,
                Err(_) => 0,
            },
            mq: self,
        }
    }
}

impl<'a> IntoIterator for &'a PosixMq {
    type Item = (u32, Vec<u8>);
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        Iter {
            max_msg_len: match self.attributes() {
                Ok(attrs) => attrs.max_msg_len,
                Err(_) => 0,
            },
            mq: self,
        }
    }
}

impl fmt::Debug for PosixMq {
    fn fmt(&self, fmtr: &mut fmt::Formatter) -> fmt::Result {
        let mut representation = fmtr.debug_struct("PosixMq");
        // display raw value and name unless we know it's a plain fd
        #[cfg(not(any(target_os = "linux", target_os = "netbsd", target_os = "dragonfly",)))]
        representation.field("mqd", &self.mqd);
        // show file descriptor where we have one
        #[cfg(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "dragonfly",
        ))]
        representation.field("fd", &self.as_raw_fd());
        representation.finish()
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
unsafe impl Sync for PosixMq {}

/// An `Iterator` that calls [`recv()`](struct.PosixMq.html#method.recv) on a borrowed [`PosixMq`](struct.PosixMq.html).
///
/// Iteration ends when a `recv()` fails with an `ErrorKind::WouldBlock` error,
/// but is infinite if the descriptor is in blocking mode.
///
/// # Panics
///
/// `next()` will panic if an error of type other than `ErrorKind::WouldBlock`
/// or `ErrorKind::Interrupted` occurs.
#[derive(Clone)]
pub struct Iter<'a> {
    mq: &'a PosixMq,
    /// Cached
    max_msg_len: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (u32, Vec<u8>);
    fn next(&mut self) -> Option<(u32, Vec<u8>)> {
        let mut buf = vec![0; self.max_msg_len];
        match self.mq.recv(&mut buf) {
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => None,
            Err(e) => panic!("Cannot receive from posix message queue: {}", e),
            Ok((priority, len)) => {
                buf.truncate(len);
                Some((priority, buf))
            }
        }
    }
}

/// An `Iterator` that [`recv()`](struct.PosixMq.html#method.recv)s
/// messages from an owned [`PosixMq`](struct.PosixMq.html).
///
/// Iteration ends when a `recv()` fails with an `ErrorKind::WouldBlock` error,
/// but is infinite if the descriptor is in blocking mode.
///
/// # Panics
///
/// `next()` will panic if an error of type other than `ErrorKind::WouldBlock`
/// or `ErrorKind::Interrupted` occurs.
pub struct IntoIter {
    mq: PosixMq,
    max_msg_len: usize,
}

impl Iterator for IntoIter {
    type Item = (u32, Vec<u8>);
    fn next(&mut self) -> Option<(u32, Vec<u8>)> {
        Iter {
            mq: &self.mq,
            max_msg_len: self.max_msg_len,
        }
        .next()
    }
}

#[cfg(debug_assertions)]
mod doctest_md_files {
    macro_rules! mdfile {($content:expr, $(#[$meta:meta])* $attach_to:ident) => {
        #[doc=$content]
        #[allow(unused)]
        $(#[$meta])* // can't #[cfg_attr(, doc=)] in .md file
        enum $attach_to {}
    }}
    mdfile! {include_str!("../README.md"), Readme}
}
