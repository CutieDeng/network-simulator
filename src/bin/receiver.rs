use std::net::UdpSocket;

mod mysocket; 

fn main() {
    let hear = UdpSocket::bind("127.0.0.2:10256").unwrap(); 
    let mut contents = [0u8; 1500]; 
    let r = mysocket::MySocket.recv(&hear, &mut contents); 
    match r {
        Some((u, s)) => {
            println!("[INFO ] Recvive from {s} info: {}", String::from_utf8_lossy(&contents[..u]));
        },
        None => {},
    }
}