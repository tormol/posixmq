use std::io;
use std::os::fd::AsRawFd;

use mio_06::unix::EventedFd;
use mio_06::{Evented, Poll, PollOpt, Ready, Token};

use crate::PosixMq;

// On FreeBSD, mqd_t is a `struct{int fd, struct sigev_node* node}*`,
// but the sigevent is only accessed by `mq_notify()`, so it's thread-safe
// as long as that function requires `&mut self` or isn't exposed.
//  src: https://svnweb.freebsd.org/base/head/lib/librt/mq.c?view=markup
// On Illumos, mqd_t points to a rather complex struct, but the functions use
// mutexes and semaphores, so I assume they're totally thread-safe.
//  src: https://github.com/illumos/illumos-gate/blob/master/usr/src/lib/libc/port/rt/mqueue.c
// Solaris I assume is equivalent to Illumos, because the Illumos code has
// barely been modified after the initial source code release.
// Linux, NetBSD and DragonFly BSD gets Sync auto-implemented because
// mqd_t is an int.
#[cfg(any(target_os = "freebsd", target_os = "illumos", target_os = "solaris"))]
unsafe impl Sync for PosixMq {}

/// Allow receiving event notifications through mio (version 0.6).
///
/// This impl requires the `mio_06` feature to be enabled:
///
/// ```toml
/// [dependencies]
/// posixmq = {version="1.0", features=["mio_06"]}
/// ```
///
/// Remember to open the queue in non-blocking mode. (with `OpenOptions.noblocking()`)
impl Evented for PosixMq {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}

impl Evented for &PosixMq {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> Result<(), io::Error> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}
