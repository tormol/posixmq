//! Receives messages from a queue, printing them and their priority to stdout.
//! Also supports timeouts or nonblocking.

use std::env::args_os;
use std::io::{stdout, Write};
use std::os::unix::ffi::OsStrExt;
use std::time::Duration;

fn main() {
    let mut args = args_os().skip(1);
    let name = args.next().expect("argument required");
    let timeout = args.next().map(|arg| {
        let arg = arg.into_string().expect("invalid timeout");
        if arg == "nonblocking" || arg == "drain" || arg == "available" || arg == "empty" {
            None
        } else if arg.ends_with("ms") {
            Some(Duration::from_millis(
                arg[..arg.len() - 2].parse().expect("invalid duration"),
            ))
        } else if arg.ends_with("s") {
            Some(Duration::from_secs(
                arg[..arg.len() - 1].parse().expect("invalid duration"),
            ))
        } else {
            Some(Duration::from_secs(arg.parse().expect("invalid duration")))
        }
    });

    let mut opts = posixmq::OpenOptions::readonly();
    if timeout == Some(None) {
        opts = *opts.nonblocking();
    }
    let mq = opts.open(name.as_bytes()).expect("opening failed");

    if let Some(Some(timeout)) = timeout {
        let mut buf = vec![0; mq.attributes().unwrap_or_default().max_msg_len];
        while let Ok((priority, len)) = mq.recv_timeout(&mut buf, timeout) {
            print!("{:3}\t", priority);
            stdout()
                .write_all(&buf[..len])
                .expect("writing to stdout failed");
            println!();
        }
    } else {
        for (priority, msg) in mq {
            print!("{:3}\t", priority);
            stdout().write_all(&msg).expect("writing to stdout failed");
            println!();
        }
    }
}
