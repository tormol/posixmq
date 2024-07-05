use std::io;
use std::os::fd::AsRawFd;

use mio_1::{event::Source, unix::SourceFd, Interest, Registry, Token};

use crate::PosixMq;

/// Allow receiving event notifications through mio (version 0.7).
///
/// This impl requires the `mio_07` feature to be enabled:
///
/// ```toml
/// [dependencies]
/// posixmq = {version="1.0", features=["mio_1"]}
/// ```
///
/// Due to a [long-lived bug in cargo]() this will currently enable
/// the os_reactor feature of mio. This is not intended, and can change in the
/// future.
///
/// You probably want to make the queue non-blocking: Either use
/// [`OpenOptions.noblocking()`](struct.OpenOptions.html#method.nonblocking)
/// when preparing to open the queue, or call [`set_nonblocking(true)`](struct.PosixMq.html#method.set_nonblocking).
impl Source for &PosixMq {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> Result<(), io::Error> {
        SourceFd(&self.as_raw_fd()).register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> Result<(), io::Error> {
        SourceFd(&self.as_raw_fd()).reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &Registry) -> Result<(), io::Error> {
        SourceFd(&self.as_raw_fd()).deregister(registry)
    }
}

impl Source for PosixMq {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> Result<(), io::Error> {
        { &mut &*self }.register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> Result<(), io::Error> {
        { &mut &*self }.reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &Registry) -> Result<(), io::Error> {
        { &mut &*self }.deregister(registry)
    }
}
