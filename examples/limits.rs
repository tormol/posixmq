//! Finds various limits and default values for posix message queues.
//! Used to find the values for one of the tables in the documentation.

/// binary search towards a limit
fn btry<P: Fn(usize) -> bool>(ceiling: usize, ok: P) -> String {
    let (mut min, mut max) = (0, ceiling);
    while min < max {
        let mid = max / 2 + min / 2;
        //eprintln!("trying {:10} ({0:#010x})", mid);
        if ok(mid) {
            min = mid + 1;
        } else {
            max = mid - 1;
        }
    }
    if !ok(min) {
        min -= 1;
    }
    if min + 1 >= ceiling {
        "No limit".to_string()
    } else {
        min.to_string()
    }
}

fn main() {
    println!("{}:", std::env::consts::OS);

    let mq = posixmq::OpenOptions::readwrite()
        .create_new()
        .open("/default_capacities")
        .expect("Cannot create new posix message queue /default_capacities");
    posixmq::remove_queue("/default_capacities")
        .expect("Cannot delete posix message queue /default_capacities");

    let attrs = mq.attributes().expect("Cannot get attributes for queue");
    println!("default queue capacity:         {}", attrs.capacity);
    println!("default maximum message length: {}", attrs.max_msg_len);

    println!(
        "max message priority:           {}",
        btry(0xff_ff_ff_ff, |priority| {
            mq.send(priority as u32, b"b")
                .map(|_| mq.recv(&mut vec![0; attrs.max_msg_len]).unwrap())
                .is_ok()
        })
    );
    println!(
        "allows empty messages:          {}",
        mq.send(0, b"").is_ok()
    );
    drop(mq);

    println!(
        "max queue capacity:             {}",
        btry(1_000_000, |capacity| {
            posixmq::OpenOptions::readwrite()
                .capacity(capacity)
                .max_msg_len(1)
                .create_new()
                .open("/max_capacity")
                .map(|_| posixmq::remove_queue("/max_capacity").unwrap())
                .is_ok()
        })
    );

    println!(
        "max message length:             {}",
        btry(1_000_000, |length| {
            posixmq::OpenOptions::readwrite()
                .max_msg_len(length)
                .capacity(1)
                .create_new()
                .open("/max_length")
                .map(|_| posixmq::remove_queue("/max_length").unwrap())
                .is_ok()
        })
    );

    println!(
        "max equal:                      {}",
        btry(1_000_000, |equal| {
            posixmq::OpenOptions::readwrite()
                .max_msg_len(equal)
                .capacity(equal)
                .create_new()
                .open("/max_equal")
                .map(|_| posixmq::remove_queue("/max_equal").unwrap())
                .is_ok()
        })
    );

    println!("allows just \"/\":                {}", {
        let result = posixmq::PosixMq::create("/");
        let _ = posixmq::remove_queue("/");
        result.is_ok()
    });
    println!("enforces name rules:            {}", {
        let noslash = std::ffi::CStr::from_bytes_with_nul(b"noslash\0").unwrap();
        let result = posixmq::OpenOptions::readwrite().create().open_c(noslash);
        let _ = posixmq::remove_queue_c(noslash);
        result.is_err()
    });
}
