use std::{sync::{Arc}, net::{SocketAddr, Ipv4Addr, SocketAddrV4}, str::FromStr};

use our_game::router::{MESSAGE_LENGTH, CACHES, Router, MessageType, GLOBAL_ROUTERS, Message};
use tokio::{runtime::Handle, net::UdpSocket};

const BUFFER_LENGTH: usize = MESSAGE_LENGTH; 

// 延迟
// todo: 主动丢包
// 收发包 bytes 单节点 track
// 压力测试
// 服务器地址固定，控制器地址
// 路由

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(7)
        .enable_all()
        .build()
        .unwrap();
    eprintln!("\x1b[32;1m[DEBUG] tokio runtime 构造完成\x1b[0m"); 
    rt.block_on(exec(rt.handle())); 
}

async fn deal(input: &str, sender: Arc<UdpSocket>) {
    let mut this = None;
    let mut value: Option<usize> = None; 
    for line in input.lines() {
        if let Some(ipv4) = line.strip_prefix("ROUTER ") {
            match Ipv4Addr::from_str(ipv4) {
                Ok(ipv4) => {
                    // let global_router = GLOBAL_ROUTERS.lock().await; 
                    this = Some(Router::from_ipv4addr(ipv4, sender.clone()).await); 
                },
                Err(_) => {
                    eprintln!("\x1b[33;1m[WARN ] unknown subcommand 'ROUTER': {ipv4}\x1b[0m"); 
                    this = None; 
                },
            }
        } else if let Some(bw) = line.strip_prefix("VALUE ") {
            value = bw.parse().ok(); 
            if let Some(0) = value { value = None; }; 
            if value.is_none() {
                eprintln!("\x1b[33;1m[WARN ] unrecognized value on subcommand 'VALUE': {bw}\x1b[0m"); 
            }
        } else if let Some(ipv4) = line.strip_prefix("LINK ") {
            match (&this, value, Ipv4Addr::from_str(ipv4)) {
                (Some(this), Some(bw), Ok(target)) => {
                    let other = Router::from_ipv4addr(target, sender.clone()).await; 
                    let mut outer = this.outers().lock().await; 
                    outer.insert(target, (bw, other.sender().clone())); 
                    drop(outer); 
                    eprintln!("\x1b[32;1m[DEBUG] successfully update a data link between {} & {}\x1b[0m", 
                        this.ipv4addr(), other.ipv4addr()); 
                }
                _ => {
                    eprintln!("\x1b[31;1m[ERROR] invalid context for subcommand 'LINK'. \x1b[0m"); 
                }
            }
        } 
        else {
            eprintln!("\x1b[33;1m[WARN ] Unknown commands: {line}\x1b[0m");
        }
    }
}

async fn exec(rt: &Handle) {
    let core_socket = UdpSocket::bind("127.67.117.116:52736").await.unwrap();
    let controller_address: SocketAddrV4 = SocketAddrV4::from_str("127.32.68.101:54528").unwrap(); 
    eprintln!("\x1b[36;1m[INFO ] server start, udp addr：{}\x1b[0m", core_socket.local_addr().unwrap()); 
    let core_socket = Arc::new(core_socket); 
    loop {
        // eprintln!("\x1b[32;1m[DEBUG] start loop\x1b[0m"); 
        let mut buffer; 
        let mut bq = CACHES.lock().await; 
        buffer = if let Some(buf) = bq.pop_back() {
            buf
        } else {
            Box::new([0u8; BUFFER_LENGTH])
        }; 
        drop(bq); 
        let (length, src) = core_socket.recv_from(buffer.as_mut_slice()).await.unwrap(); 
        eprintln!("\x1b[32;1m[DEBUG] recv a packet from {src}\x1b[0m");
        match src {
            SocketAddr::V4(a) if a == controller_address => {
                let words = std::str::from_utf8(&buffer[0..length]);
                match words {
                    Ok(words) => {
                        deal(words, core_socket.clone()).await; 
                    },
                    Err(_) => {
                        eprintln!("\x1b[31;1m[ERROR] Message from controller can't be recognized. \x1b[0m"); 
                    },
                }
            },
            SocketAddr::V4(client) => {
                rt.spawn(async move {
                    push_in_network(buffer, length, client).await; 
                }); 
            }
            _ => {
                eprintln!("\x1b[33;1m[WARN ] Ipv6 Unsupported: receive a packet from ipv6 internet. \x1b[0m"); 
            }, 
        }
    }
}

pub async fn push_in_network(mut buffer: MessageType, message_length: usize, from_ip: SocketAddrV4) {
    assert! (buffer.len() >= message_length); 
    if message_length < 6 {
        eprintln!("\x1b[33;1m[WARN ] packet too short, cannot determine the target. "); 
        return 
    }
    eprintln!("\x1b[32;1m[DEBUG] Prepare this packet from {from_ip}\x1b[0m"); 
    let global_router = GLOBAL_ROUTERS.lock().await; 
    let r; 
    match global_router.get(from_ip.ip()) {
        Some(router) => {
            r = router.clone(); 
        },
        None => {
            eprintln!("\x1b[33;1m[WARN ] cannot found the related router to deliver the packet. "); 
            return ; 
        },
    }
    drop(global_router); 
    let target_addr = SocketAddrV4::new(Ipv4Addr::new(buffer[0], buffer[1], buffer[2], buffer[3]), 
        buffer[4] as u16 + (( buffer[5] as u16 ) << 8)); 
    let src_ip = from_ip.ip().octets();
    for i in 0..4 {
        buffer[i] = src_ip[i]; 
    }
    buffer[4] = from_ip.port() as u8; 
    buffer[5] = (from_ip.port() >> 8) as u8; 
    let message: Message = Message { target: target_addr, message: buffer, message_len: message_length }; 
    eprintln!("\x1b[32;1m[DEBUG] construct the packet to {} and push into network. \x1b[0m", target_addr); 
    r.sender().send(message).unwrap();
}