//! Get the default capacities for posix message queues.

extern crate posixmq;

fn main() {
    let attrs = posixmq::OpenOptions::readwrite()
        .create_new()
        .open("/default_capacities")
        .expect("Cannot create new posix message queue /default_capacities")
        .attributes();
    println!("default queue capacity:         {}", attrs.capacity);
    println!("default maximum message length: {}", attrs.max_msg_len);
    posixmq::unlink("/default_capacities")
        .expect("Cannot delete posix message queue /default_capacities");
}
