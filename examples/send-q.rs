use std::net::UdpSocket;

// Example 1: send and receive in the same router: 127.6.6.6 (6666 -> 6665) 
// sender: send-q.py 
// recvive: recv-q.py 

fn main() {
    let sender = UdpSocket::bind("127.32.68.101:54528").unwrap(); 
    eprintln!("run the controller for 'q' target. "); 
    sender.send_to("ROUTER 127.6.6.6".as_bytes(), "127.67.117.116:52736").unwrap(); 
}