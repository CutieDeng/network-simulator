use std::net::{UdpSocket, ToSocketAddrs, SocketAddr, SocketAddrV4, Ipv4Addr};

pub struct MySocket; 

impl MySocket {
    pub fn send (&self, proxy: &UdpSocket, send_to: impl ToSocketAddrs, content: &[u8]) -> Result<(), ()> {
        let mut new_contents = Vec::new(); 
        new_contents.reserve(content.len() + 6); 
        match send_to.to_socket_addrs() {
            Ok(mut o) => {
                let o = o.next(); 
                if let Some(o) = o {
                    match o {
                        std::net::SocketAddr::V4(v) => {
                            let octet = v.ip().octets();
                            for o in octet {
                                new_contents.push(o); 
                            }
                            let port = v.port(); 
                            new_contents.push(port as u8); 
                            new_contents.push((port >> 8) as u8); 
                        }
                        std::net::SocketAddr::V6(e) => {
                            eprintln!("\x1b[31;1m[Error] Cannot parse ipv6 {e} in 6 bytes..."); 
                            return Err(())
                        }
                    }
                }
            },
            Err(e) => {
                eprintln!("\x1b[31;1m[Error] {e:?}\x1b[0m"); 
                return Err(())
            },
        }
        new_contents.extend_from_slice(content);
        match proxy.send_to(&new_contents, "127.0.0.1:9999") {
            Ok(c) => {
                eprintln!("[Debug] Successfully send packet, with {c} bytes. ")
            },
            Err(e) => {
                eprintln!("\x1b[31;1m[Error] Cannot find the core proxy server: {e:?} \x1b[0m"); 
                return Err(())
            },
        }
        Ok(())
    }
    pub fn recv(&self, proxy: &UdpSocket, content: &mut [u8]) -> Option<(usize, SocketAddr)> {
        let r = proxy.recv_from(content); 
        match r {
            Ok((len, _)) => {
                if len < 6 {
                    return None
                }
                let sock_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(content[0], content[1], content[2], content[3]), content[4] as u16 + content[5] as u16 * 0x100)); 
                for index in 0..(len-6) {
                    content[index] = content[index+6]; 
                }
                return Some((len - 6, sock_addr))
            },
            Err(_) => { return None },
        }
    }
}