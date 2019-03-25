//! Receives messages from a queue, printing them and their priority to stdout.

extern crate posixmq;

use std::env::args_os;
use std::io::{stdout, Write};
use std::os::unix::ffi::OsStrExt;

fn main() {
    let name = args_os().skip(1).next().expect("argument required");
    let name = name.as_bytes();

    let mq = posixmq::PosixMq::open(name).expect("opening failed");

    let mut buf = [0; 8192];
    loop {
        let (priority, len) = mq.receive(&mut buf).expect("receiving failed");
        print!("{:3}\t", priority);
        stdout().write_all(&buf[..len]).expect("writing to stdout failed");
        println!();
    }
}
