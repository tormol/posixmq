/* Copyright 2019 Torbjørn Birch Moltu
 *
 * Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
 * http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
 * http://opensource.org/licenses/MIT>, at your option. This file may not be
 * copied, modified, or distributed except according to those terms.
 */

//! Posix message queue wrapper with optional mio support.
//!
//! Posix message queues are like pipes, but message-oriented and the messages
//! are sorted based on an additional priority parameter.
//! For a longer introduction, see `man mq_overview`.
//!
//! They are not all that useful, as only Linux and some BSDs implement them,
//! and on Linux they are limitied by default to a capacity of only ten messages.
//!
//! See the examples/ directory for examples.
//!
//! # Supported operating systems
//!
//! * Linux >= 2.6.26: all features, tested on CI.
//! * FreeBSD 11+: everything except `FromRawFd` and `IntoRawFd`, tested on CI.
// (11 added a function which made `AsRawFd` and thereby mio `Evented` possible)
// [r306588](https://github.com/freebsd/freebsd/commit/6e61756bbf70) merged on 2016-10-09
//! * NetBSD: untested but all features cross-compile and might work.
//! * Fuchsia: untested but default features cross-compile.
//! * DragonFlyBSD: untested, only basic features (no `AsRawFd` or mio integration)
//! * Solaris / Illumos: The OS should have them, but libc doesn't export symbols
//! * Not Windows, macOS, Android, OpenBSD or Rumprun. (doesn't have posix message queues)
//! * Other: [maybe if the libc crate has the symbols](https://github.com/rust-lang/libc/search?q=mq_open&unscoped_q=mq_open) core features might work.
//!
//! This library will try to compile most features even if the target OS
//! doesn't support posix message queues. `AsRawFd`, `FromRawFd` and `IntoRawFd`
//! are implemented as casts for all platforms where `mqd_t` is an `int`, based
//! on the asumption that it's a file descriptor, which is the case on Linux.
//!
//! # Differences from the C API
//!
//! * `send()` and `receive()` tries again when EINTR / `ErrorKind::Interrupted`
//!   is returned. (Consistent with normal Rust io)
//! * Descriptors are by default opened with O_CLOEXEC. (Consistent with normal Rust io)
//! * `open()` and all other methods which take `AsRef<[u8]>` prepends '/' to
//!   the name if missing. (They allocate anyway, to append a terminating '\0')
//!
//! # Missing features / wishlist / TODO
//!
//! * `send_timeout()` (`mq_timedsend()`)
//! * `receive_timeout()` (`mq_timedreceive()`)
//! * `try_clone()`
//! * `Iterator` struct that calls `receive()`
//! * Listing queues and their owners using OS-specific interfaces
//!   (such as /dev/mqueue/ on Linux)
//! * Getting and possibly changing limits and default values
//! * Struct that deletes the message queue when dropped
//! * Test or check more platforms on CI
//! * Examples in the documentation
//! * Support more OSes?
//! * `mq_notify()`?
//!
//! Please open an issue (or pull request) if you want any of them.
//
// # Why no `#![no_std]`-mode
//
// The libc crate doesn't expose `errno` in a portable way,
// so `std::io::Error::last_err()` is required. Depending on std also means the
// functions can return `io::Error` instead of some custom type.

use std::{io, mem, ptr};
use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::io::ErrorKind;
use std::fmt::{self, Debug, Formatter};
#[cfg(not(target_os="dragonflybsd"))]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(not(any(target_os="freebsd", target_os="dragonflybsd")))]
use std::os::unix::io::{FromRawFd, IntoRawFd};

extern crate libc;
use libc::{c_int, c_uint, c_long, mode_t};
use libc::{mqd_t, mq_open, mq_send, mq_receive, mq_close, mq_unlink};
use libc::{mq_attr, mq_getattr, mq_setattr};
use libc::{O_ACCMODE, O_RDONLY, O_WRONLY, O_RDWR};
use libc::{O_CREAT, O_EXCL, O_NONBLOCK, O_CLOEXEC};
use libc::{fcntl, F_GETFD, F_SETFD, FD_CLOEXEC};

#[cfg(target_os="freebsd")]
extern "C" {
    // not in libc (yet)
    fn mq_getfd_np(mq: mqd_t) -> c_int;
}

#[cfg(feature="mio")]
extern crate mio;
#[cfg(feature="mio")]
use mio::event::Evented;
#[cfg(feature="mio")]
use mio::unix::EventedFd;
#[cfg(feature="mio")]
use mio::{Ready, Poll, PollOpt, Token};


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
    if name.len() > 0  &&  name[0] == b'/'  &&  name[name.len()-1] == b'\0' {
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


// Cannot use std::fs's because it doesn't expose getters,
// and rolling our own means we can also use it for mq-specific capacities.
/// Flags and parameters which control how a [`PosixMq`](struct.PosixMq.html)
/// message queue is opened or created.
#[derive(Clone,Copy, PartialEq,Eq)]
pub struct OpenOptions {
    mode: c_int,
    permissions: mode_t,
    capacity: usize,
    max_msg_len: usize,
}

impl Debug for OpenOptions {
    fn fmt(&self,  fmtr: &mut Formatter) -> fmt::Result {
        fmtr.debug_struct("OpenOptions")
            .field("read", &((self.mode & O_ACCMODE) == O_RDWR || (self.mode & O_ACCMODE) == O_RDONLY))
            .field("write", &((self.mode & O_ACCMODE) == O_RDWR || (self.mode & O_ACCMODE) == O_WRONLY))
            .field("create", &(self.mode & O_CREAT != 0))
            .field("open", &(self.mode & O_EXCL == 0))
            .field("permissions", &format_args!("{:03o}", self.permissions))
            .field("capacity", &self.capacity)
            .field("max_msg_len", &self.max_msg_len)
            .field("nonblocking", &((self.mode & O_NONBLOCK) != 0))
            .field("cloexec", &((self.mode & O_CLOEXEC) != 0))
            .finish()
    }
}

impl OpenOptions {
    fn new(mode: c_int) -> Self {
        OpenOptions {
            // std sets cloexec unconditionally as a security feature
            // root issue: https://github.com/rust-lang/rust/issues/12148
            mode: O_CLOEXEC | mode,
            // default permissions to only accessible for owner
            permissions: 0o700,
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
    /// This field is ignored if the queue already exists or should not be created.
    /// If this method is not called, queues are created with permissions 700.
    pub fn permissions(&mut self,  permissions: u16) -> &mut Self {
        self.permissions = permissions as mode_t;
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
        self.mode |= O_CREAT;
        self.mode &= !O_EXCL;
        return self;
    }

    /// Create a new queue, failing if the queue already exists.
    pub fn create_new(&mut self) -> &mut Self {
        self.mode |= O_CREAT | O_EXCL;
        return self;
    }

    /// Require the queue to already exist, failing if it doesn't.
    pub fn existing(&mut self) -> &mut Self {
        self.mode &= !(O_CREAT | O_EXCL);
        return self;
    }

    /// Open the message queue in non-blocking mode.
    ///
    /// This must be done if you want to use the message queue with mio.
    pub fn nonblocking(&mut self) -> &mut Self {
        self.mode |= O_NONBLOCK;
        return self;
    }

    /// Keep the message queue open after `exec`ing into another program.
    pub fn not_cloexec(&mut self) -> &mut Self {
        self.mode &= !O_CLOEXEC;
        return self;
    }

    /// Open a queue with the specified options.
    ///
    /// # Errors
    ///
    /// * Queue doesn't exist (ENOENT) => `ErrorKind::NotFound`
    /// * Name is just "/" (ENOENT) or is empty => `ErrorKind::NotFound`
    /// * Queue already exists (EEXISTS) => `ErrorKind::AlreadyExists`
    /// * Not permitted to open in this mode (EACCESS) => `ErrorKind::PermissionDenied`
    /// * More than one '/' in name (EACCESS) => `ErrorKind::PermissionDenied`
    /// * Invalid capacities (EINVAL) => `ErrorKind::InvalidInput`
    /// * Name contains '\0' => `ErrorKind::InvalidInput`
    /// * Name is too long (ENAMETOOLONG) => `ErrorKind::Other`
    /// * Unlikely (ENFILE, EMFILE, ENOMEM, ENOSPC) => `ErrorKind::Other`
    /// * Possibly other
    pub fn open<N: AsRef<[u8]> + ?Sized>(&self,  name: &N) -> Result<PosixMq, io::Error> {
        name_to_cstring(name.as_ref()).and_then(|name| self.open_c(&name) )
    }

    /// Open a queue with the specified options and without inspecting `name`.
    ///
    /// # Errors
    ///
    /// * Queue doesn't exist (ENOENT) => `ErrorKind::NotFound`
    /// * Name is just "/" (ENOENT) => `ErrorKind::NotFound`
    /// * Queue already exists (EEXISTS) => `ErrorKind::AlreadyExists`
    /// * Not permitted to open in this mode (EACCESS) => `ErrorKind::PermissionDenied`
    /// * More than one '/' in name (EACCESS) => `ErrorKind::PermissionDenied`
    /// * Invalid capacities (EINVAL) => `ErrorKind::InvalidInput`
    /// * Name is empty (EINVAL) => `ErrorKind::InvalidInput`
    /// * Name is too long (ENAMETOOLONG) => `ErrorKind::Other`
    /// * Unlikely (ENFILE, EMFILE, ENOMEM, ENOSPC) => `ErrorKind::Other`
    /// * Possibly other
    pub fn open_c(&self,  name: &CStr) -> Result<PosixMq, io::Error> {
        PosixMq::new_c(name, self)
    }
}


/// Delete a posix message queue.
///
/// # Errors
///
/// * Queue doesn't exist (ENOENT) => `ErrorKind::NotFound`
/// * Name is just "/" (ENOENT) or is empty => `ErrorKind::NotFound`??
/// * Not permitted to delete the queue (EACCES) => `ErrorKind::PermissionDenied`
/// * More than one '/' in name (EACCESS) => `ErrorKind::PermissionDenied`
/// * Name contains '\0' bytes => `ErrorKind::InvalidInput`
/// * Name is too long (ENAMETOOLONG) => `ErrorKind::Other`
/// * Possibly other
pub fn unlink<N: AsRef<[u8]> + ?Sized>(name: &N) -> Result<(), io::Error> {
    name_to_cstring(name.as_ref()).and_then(|name| unlink_c(&name) )
}

/// Delete a posix message queue, without inspecting `name`.
///
/// # Errors
///
/// * Queue doesn't exist (ENOENT) => `ErrorKind::NotFound`
/// * Name is just "/" (ENOENT) => `ErrorKind::NotFound`??
/// * Not permitted to delete the queue (EACCES) => `ErrorKind::PermissionDenied`
/// * More than one '/' in name (EACCESS) => `ErrorKind::PermissionDenied`
/// * Name is empty or does not start with a slash (EINVAL) => `ErrorKind::InvalidInput`
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


// The fields of `mq_attr` are of type `long` on all targets except
// x86_64-unknown-linux-gnux32, where they are `long long` (to match up with
// normal x86_64 `long`).
// Rusts lack of implicit widening makes this peculiarity annoying.
#[cfg(target="x86_64-unknown-linux-gnux32")]
type AttrField = i64;
#[cfg(not(target="x86_64-unknown-linux-gnux32"))]
type AttrField = c_long;

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
    fn new_c(name: &CStr,  opts: &OpenOptions) -> Result<Self, io::Error> {
        // because mq_open is a vararg function, mode_t cannot be passed
        // directly on FreeBSD where it's smaller than c_int.
        let permissions = opts.permissions as c_int;

        let mut capacities = unsafe { mem::zeroed::<mq_attr>() };
        let mut capacities_ptr = ptr::null_mut::<mq_attr>();
        if opts.capacity != 0 || opts.max_msg_len != 0 {
            capacities.mq_maxmsg = opts.capacity as AttrField;
            capacities.mq_msgsize = opts.max_msg_len as AttrField;
            capacities_ptr = &mut capacities as *mut mq_attr;
        }

        let mqd = unsafe { mq_open(name.as_ptr(), opts.mode, permissions, capacities_ptr) };
        // even when mqd_t is a pointer, -1 is the return value for error
        if mqd == -1isize as mqd_t {
            return Err(io::Error::last_os_error());
        }

        let mq = PosixMq{mqd};

        // close-on-exec is enabled even without O_CLOEXEC on both Linux and FreeBSD
        // TODO optimize by storing platform behaviour in a global atomic variable
        // Ignore errors; It is unlikely to fail, in most cases it doesn't matter,
        // and if open() created a queue, the caller must be able to differentiate.
        #[cfg(not(target_os="dragonflybsd"))]
        let _ = unsafe { mq.set_cloexec(opts.mode & O_CLOEXEC != 0) };

        // TODO check if O_NONBLOCK was actually set (NetBSD ignores it)
        Ok(mq)
    }

    /// Open an existing message queue in read-only mode.
    ///
    /// See [`OpenOptions::open()`](struct.OpenOptions.html#method.open) for
    /// details and possible errors.
    pub fn open<N: AsRef<[u8]> + ?Sized>(name: &N) -> Result<Self, io::Error> {
        OpenOptions::readonly().open(name)
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
    /// # Errors
    ///
    /// * Queue is full and opened in nonblocking mode (EAGAIN) => `ErrorKind::WouldBlock`
    /// * The message is too big for the queue (EMSGSIZE) => `ErrorKind::Other`
    /// * Possibly other => `ErrorKind::Other`
    pub fn send(&self,  priority: u32,  msg: &[u8]) -> Result<(), io::Error> {
        let bptr = msg.as_ptr() as *const i8;
        loop {// catch EINTR and retry
            let ret = unsafe { mq_send(self.mqd, bptr, msg.len(), priority as c_uint) };
            if ret == 0 {
                return Ok(());
            }
            let err = io::Error::last_os_error();
            if err.kind() != ErrorKind::Interrupted {
                return Err(err)
            }
        }
    }

    /// Take the message with the highest priority from the queue.
    ///
    /// # Errors
    ///
    /// * Queue is empty and opened in nonblocking mode (EAGAIN) => `ErrorKind::WouldBlock`
    /// * The receive buffer is smaller than the queue's maximum message size (EMSGSIZE) => `ErrorKind::Other`
    /// * Possibly other => `ErrorKind::Other`
    pub fn receive(&self,  msgbuf: &mut [u8]) -> Result<(u32, usize), io::Error> {
        let bptr = msgbuf.as_mut_ptr() as *mut i8;
        let mut priority = 0 as c_uint;
        loop {// catch EINTR and retry
            let len = unsafe { mq_receive(self.mqd, bptr, msgbuf.len(), &mut priority) };
            if len >= 0 {
                // c_uint is unlikely to differ from u32, but even if it's bigger, the
                // range of supported values will likely be far smaller.
                return Ok((priority as u32, len as usize));
            }
            let err = io::Error::last_os_error();
            if err.kind() != ErrorKind::Interrupted {
                return Err(err)
            }
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
                nonblocking: (attrs.mq_flags & (O_NONBLOCK as AttrField)) != 0,
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
        attrs.mq_flags = if nonblocking {O_NONBLOCK as AttrField} else {0};
        let res = unsafe { mq_setattr(self.mqd, &attrs, ptr::null_mut()) };
        if res == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }


    /// Check whether this descriptor will be closed if the process `exec`s
    /// into another program.
    ///
    /// # Errors
    ///
    /// Retrieving this flag should only fail if the queue is already closed.
    /// In that case `true` is returned because the queue will not be open
    /// after `exec`ing.
    #[cfg(not(target_os="dragonflybsd"))]
    pub fn is_cloexec(&self) -> bool {
        let flags = unsafe { fcntl(self.as_raw_fd(), F_GETFD) };
        if flags == -1 {
            true
        } else {
            (flags & FD_CLOEXEC) != 0
        }
    }

    /// Set close-on-exec for this descriptor.
    ///
    /// `PosixMq` enables close-on-exec by default when opening message queues,
    /// but this can be disabled with `OpenOptions::not_cloexec()`.
    /// Prefer using `OpenOptions` to set it, because another thread might
    /// `exec` between the message queue being opened and this change taking
    /// effect.
    ///
    /// Additionally, this function has a race condition with itself, as the
    /// flag cannot portably be set atomically without affecting other attributes.
    ///
    /// # Errors
    ///
    /// This function should only fail if the underlying file descriptor has
    /// been closed (due to incorrect usage of `from_raw_fd()` or similar),
    /// and not reused for something else yet.
    #[cfg(not(target_os="dragonflybsd"))]
    pub unsafe fn set_cloexec(&self,  cloexec: bool) -> Result<(), io::Error> {
        // Race-prone but portable; Linux and the BSDs have fcntl(, F_IOCLEX)
        // but fuchsia and solarish doesn't.
        // https://github.com/rust-lang/rust/blob/master/src/libstd/sys/unix/fd.rs#L173
        let prev = fcntl(self.as_raw_fd(), F_GETFD);
        if prev == -1 {
            // Don't hide the error here, because callers can ignore the
            // returned value if they want.
            return Err(io::Error::last_os_error());
        }
        let new = if cloexec {
            prev | FD_CLOEXEC
        } else {
            prev & !FD_CLOEXEC
        };
        if new != prev {
            let ret = fcntl(self.as_raw_fd(), F_SETFD, new);
            if ret == -1 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
}

/// Get the raw file descriptor for the queue.
///
/// Note that the queue will be closed when the returned `PosixMq` goes out
/// of scope / is dropped.
///
/// This impl is not available on DragonFlyBSD.
#[cfg(not(target_os="dragonflybsd"))]
impl AsRawFd for PosixMq {
    // On Linux, `mqd_t` is a plain file descriptor and can trivially be convverted,
    // but this is not guaranteed, nor the case on FreeBSD or DragonFlyBSD.
    #[cfg(not(any(target_os="freebsd", target_os="dragonflybsd")))]
    fn as_raw_fd(&self) -> RawFd {
        self.mqd as RawFd
    }

    // FreeBSD has mq_getfd_np() (where _np stands for non-portable)
    #[cfg(target_os="freebsd")]
    fn as_raw_fd(&self) -> RawFd {
        unsafe { mq_getfd_np(self.mqd) as RawFd }
    }
}

/// Create a `PosixMq` handle from a raw file descriptor.
///
/// Note that the queue will be closed when the returned `PosixMq` goes out
/// of scope / is dropped.
#[cfg(not(any(target_os="freebsd", target_os="dragonflybsd")))]
impl FromRawFd for PosixMq {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        PosixMq { mqd: fd as mqd_t }
    }
}

/// Convert the `PosixMq` into a raw file descriptor.
#[cfg(not(any(target_os="freebsd", target_os="dragonflybsd")))]
impl IntoRawFd for PosixMq {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.mqd;
        mem::forget(self);
        return fd;
    }
}

impl Debug for PosixMq {
    #[cfg(any(target_os="linux", target_os="freebsd"))]
    fn fmt(&self,  fmtr: &mut Formatter) -> fmt::Result {
        write!(fmtr, "PosixMq{{ fd: {} }}", self.as_raw_fd())
    }

    #[cfg(not(any(target_os="linux", target_os="freebsd")))]
    fn fmt(&self,  fmtr: &mut Formatter) -> fmt::Result {
        write!(fmtr, "PosixMq{{}}")
    }
}

impl Drop for PosixMq {
    fn drop(&mut self) {
        unsafe { mq_close(self.mqd) };
    }
}


/// Make posix message queues pollable by mio.
///
/// This impl requires the `mio` feature to be enabled:
///
/// ```toml
/// [dependencies]
/// posixmq = {version="0.1", features="mio"}
/// ```
///
/// Remember to open the queue in non-blocking mode. (with `OpenOptions.noblocking()`)
#[cfg(feature="mio")]
impl Evented for PosixMq {
    fn register(&self,  poll: &Poll,  token: Token,  interest: Ready,  opts: PollOpt)
    -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(&self,  poll: &Poll,  token: Token,  interest: Ready,  opts: PollOpt)
    -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self,  poll: &Poll) -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}


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
