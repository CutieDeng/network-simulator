use std::net::UdpSocket;

mod mysocket; 

fn main() {
    let p = mysocket::MySocket.send(&UdpSocket::bind("127.0.0.1:10257").unwrap(), "127.0.0.2:10256",
        "This is ".as_bytes());
    eprintln!("{p:?}");
}
