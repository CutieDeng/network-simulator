use std::net::UdpSocket;

// Example 2: send and receive in one link (127.4.4.4 -> 127.4.5.5 -> 127.4.5.6)
// sender: delay-s.py 
// recvive: delay-r.py

fn main() {
    let sender = UdpSocket::bind("127.32.68.101:54528").unwrap(); 
    eprintln!("run the controller for 'delay' target. "); 
    let info = r"ROUTER 127.4.4.4
VALUE 32
LINK 127.4.5.5
ROUTER 127.4.5.5
VALUE 32
LINK 127.4.5.6"; 
    sender.send_to(info.as_bytes(), "127.67.117.116:52736").unwrap(); 
}