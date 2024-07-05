//! Simple utility for deleting posix message queues.
//! Useful to clean up when something doesn't clean up after itself.

use std::env::args_os;
use std::os::unix::ffi::OsStrExt;

fn main() {
    for arg in args_os().skip(1) {
        if let Err(e) = posixmq::remove_queue(arg.as_bytes()) {
            let name = arg.to_string_lossy();
            eprintln!("Cannot remove {}: {}", name, e);
        }
    }
}
