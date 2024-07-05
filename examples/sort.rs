//! Uses a queue to sort command line arguments based on thir length.

use std::env::args_os;
use std::ffi::CStr;
use std::io::{stdout, ErrorKind, Write};
use std::os::unix::ffi::OsStringExt;

fn main() {
    let args = args_os()
        .skip(1)
        .map(|osstring| osstring.into_vec())
        .collect::<Vec<Vec<u8>>>();
    if args.is_empty() {
        return; // nothing to do
    }

    let mut recv_buf = vec![0; args.iter().map(Vec::len).max().unwrap()];

    let name = CStr::from_bytes_with_nul(b"/sort\0").unwrap();

    // create queue with the necessary permissions and open it
    let mq = posixmq::OpenOptions::readwrite()
        .nonblocking() // use WouldBlock to detect that the queue is empty
        .create_new()
        .mode(0o000) // only affects future attempts at opening it
        .capacity(args.len())
        .max_msg_len(recv_buf.len())
        .open_c(name)
        .expect("opening queue failed");

    // write arguments to the queue
    for arg in args {
        mq.send(arg.len() as u32, &arg).expect("sending failed");
    }

    // read until the queue is empty
    let stdout = stdout();
    let mut stdout = stdout.lock();
    loop {
        match mq.recv(&mut recv_buf) {
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) => panic!("receiving failed: {}", e),
            Ok((_priority, len)) => {
                stdout
                    .write_all(&recv_buf[..len])
                    .and_then(|()| stdout.write_all(b"\n"))
                    .expect("writing to stdout failed");
            }
        }
    }

    // clean up
    posixmq::remove_queue_c(name).expect("deleting queue failed")
}
