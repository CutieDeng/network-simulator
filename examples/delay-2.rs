use std::net::UdpSocket;

// Example 3: mostly based on example 2 
// extend the size of the link bitwidth.. 

fn main() {
    let sender = UdpSocket::bind("127.32.68.101:54528").unwrap(); 
    eprintln!("run the controller for 'delay' target. "); 
    let info = r"ROUTER 127.4.4.4
VALUE 3200
LINK 127.4.5.5
ROUTER 127.4.5.5
VALUE 3200
LINK 127.4.5.6"; 
    sender.send_to(info.as_bytes(), "127.67.117.116:52736").unwrap(); 
}