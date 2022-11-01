use std::net::UdpSocket;

static TEXT: &'static str = include_str!("input.txt"); 

fn main() {
    let controller = UdpSocket::bind("127.32.68.101:54528").unwrap(); 
    controller.send_to(TEXT.as_bytes(), "127.67.117.116:52736").unwrap(); 
}