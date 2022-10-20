use std::net::UdpSocket;

use our_game::mysocket;

fn main() {
    let p = mysocket::MySocket.send(&UdpSocket::bind("127.0.0.1:10257").unwrap(), "127.0.0.1:10256",
        "This is a quite long long long long long bytes for you to read... ".as_bytes());
    eprintln!("{p:?}");
}