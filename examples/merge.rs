//! Receive messages from multiple queues and send them to another,
//! asynchronously.

extern crate posixmq;
#[cfg(feature="mio")]
extern crate mio;

#[cfg(feature="mio")]
fn main() {
    use std::env::args;
    use std::io::ErrorKind;

    use mio::{Poll, Events, PollOpt, Ready, Token};

    let mut queues = args().skip(1).collect::<Vec<_>>();
    let dst = queues.pop().expect("arguments required");
    
    // open source queues
    let mut src = Vec::new();
    for name in queues {
        match posixmq::OpenOptions::readonly().nonblocking().open(&name) {
            Ok(mq) => src.push((mq, name)),
            Err(e) => panic!("Cannot open {:?} for receiving: {}", name, e),
        }
    }

    // open destination queue
    let dst = match posixmq::OpenOptions::writeonly().nonblocking().create().open(&dst) {
        Ok(mq) => (mq, dst),
        Err(e) => panic!("Cannot open or create {:?} for sending: {}", dst, e),
    };

    let poll = Poll::new().expect("Cannot create selector");
    poll.register(&dst.0, Token(0), Ready::writable(), PollOpt::edge())
        .expect("registering destination failed");
    for (i, &(ref src, _)) in src.iter().enumerate() {
        poll.register(src, Token(i+1), Ready::readable(), PollOpt::edge())
            .expect("registering a source failed");
    }

    let mut unsent = Vec::<(u32, Box<[u8]>, &str)>::new();
    let mut buf = [0; 8192];
    let mut events = Events::with_capacity(1024);
    loop {
        poll.poll(&mut events, None).expect("Cannot poll selector");

        // receive all available messages from queues that are ready
        for event in events.iter() {
            if event.token() == Token(0) {
                // dst; will try to send below even without this event
                continue;
            }

            let &(ref mq, ref name) = &src[event.token().0-1];
            loop {
                match mq.receive(&mut buf) {
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) => panic!("Error receiving from {}: {}", name, e),
                    Ok((priority, len)) => unsent.push((priority, Box::from(&buf[..len]), name)),
                }
            }
        }

        // send as many messages as possible
        while let Some(&(priority, ref msg, ref src)) = unsent.last() {
            if let Err(e) = dst.0.send(priority, msg) {
                if e.kind() == ErrorKind::WouldBlock {
                    break;
                }
                panic!("Error sending to {}: {}", dst.1, e);
            }
            println!("message of priority {} with {} bytes from {} sent to {}", 
                priority, msg.len(), src, dst.1
            );
            let _ = unsent.pop();
        }
    }
}

// to make the example compile when chehcked as part of `cargo test`
#[cfg(not(feature="mio"))]
fn main() {
    panic!("This example require --features mio")
}