//! reads messages from stdin or the command line and sends them to a queue.

use std::env::args_os;
use std::io::{stdin, BufRead};
use std::os::unix::ffi::OsStrExt;

fn main() {
    let args = args_os().skip(1).collect::<Vec<_>>();
    if args.len() % 2 == 0 {
        eprintln!("Usage: cargo run --example send /queue-name [priority message] ...");
        return;
    }

    // open the message queue
    let mq = posixmq::OpenOptions::writeonly()
        .create()
        .mode(0o666) // FIXME set umask to actually make it sendable for others
        .open(args[0].as_bytes())
        .expect("opening failed");

    if args.len() == 1 {
        // read from stdin
        for line in stdin().lock().lines() {
            let line = line.expect("input must be UTF-8, sorry");
            let line = line.trim_start();
            let num_end = line.find(' ').expect("no space in line");
            let priority = line[..num_end]
                .parse::<u32>()
                .expect("priority is not a number");
            let msg = line[num_end + 1..].trim_start();
            mq.send(priority, msg.as_bytes()).expect("sending failed");
        }
    } else {
        // read from the command line
        for i in (1..args.len()).step_by(2) {
            let priority = args[i]
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
                .expect("priority is not a number");
            let msg = args[i + 1].as_bytes();
            mq.send(priority, msg).expect("sending failed");
        }
    }
}
