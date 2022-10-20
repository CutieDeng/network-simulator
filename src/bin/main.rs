use std::{net::{UdpSocket, SocketAddr, IpAddr, Ipv4Addr}, sync::Mutex, io::ErrorKind, collections::BTreeMap, thread::sleep, time::Duration};
use lazy_static::lazy_static;

use our_game::ThreadPool;

const BUFFER_SIZE : usize = 25;  
type BufferArray = [u8; BUFFER_SIZE]; 

static BUFFERS: Mutex<Vec<Box<BufferArray>>> = Mutex::new(Vec::new()); 
lazy_static! {
    static ref LINKS_BITWIDTH : Mutex<BTreeMap<(SocketAddr, SocketAddr), usize>> = Mutex::new(BTreeMap::new()); 
}

fn forward(buffer: &mut [u8], pid: usize, addr: SocketAddr, sender: UdpSocket) -> Result<(), ()> {
    if buffer.len() < 6 {
        eprintln!("\x1b[31;1m[Error] Receive a packet[id {pid}] whose length is less than 6, would be dropped directly. \x1b[0m");
        return Err(())
    }
    eprintln!("\x1b[32;1m[Debug] Start forward packet[id {pid}]. \x1b[0m",);
    let target = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(buffer[0], buffer[1], buffer[2], buffer[3])), 
        ( buffer[5] as u16 ) << 8 | buffer[4] as u16 ); 
    match addr {
        SocketAddr::V4(v4) => {
            for i in 0..4 {
                buffer[i] = v4.ip().octets()[i]; 
            }
            buffer[4] = v4.port() as u8; 
            buffer[5] = (v4.port() >> 8) as u8;  
        }
        SocketAddr::V6(v6) => {
            eprintln!("\x1b[31;1m[Error] Packet[id {pid}] is from ipv6 {v6}. \x1b[0m"); 
            return Err(())
        }
    }
    let link_bw = LINKS_BITWIDTH.lock().unwrap(); 
    let p = link_bw.get(&(addr, target)).map(|f| *f); 
    drop(link_bw); 
    // let bitwidth; 
    match p {
        Some(bw) => {
            // bitwidth = bw; 
            let time_cost = (buffer.len() + 6) * 8; 
            if bw == 0 {
                eprintln!("\x1b[33;1m[Warn ] Packet[id {pid}] cannot sent: bitwidth equals 0! \x1b[0m"); 
                return Err(())
            }
            let time_cost = time_cost / bw + 1; 
            sleep(Duration::from_millis(time_cost as u64)); 
        },
        None => {
            eprintln!("\x1b[33;1m[Warn ] Packet[id {pid}] drops: no links between {addr} & {target}\x1b[0m"); 
            return Err(())
        },
    }
    eprintln!("\x1b[36;1m[Info ] Packet[id {pid}] target addr: {target}. \x1b[0m"); 
    let d = sender.send_to(&buffer, target);
    let d = d.unwrap(); 
    if d != buffer.len() {
        eprintln!("\x1b[33;1m[Warn ] Packet[id {pid}] only be sent {d} bytes, but total bytes: {}.\x1b[0m", buffer.len())
    }
    Ok(())
} 

fn main() {
    // 构造 UDP 监听服务器
    let udp_server = UdpSocket::bind("localhost:9999").unwrap(); 
    let mut id: usize = 0; 
    eprintln!("\x1b[36;1m[Info ] Bind the UDP Socket at {} successfully. \x1b[0m", udp_server.local_addr().unwrap()); 
    // 构造线程池
    let (_, task_manager) = ThreadPool::with_thread_num(7); 
    eprintln!("\x1b[36;1m[Info ] Construct the thread pool to handle data. \x1b[0m"); 
    loop {
        let mut b = BUFFERS.lock().unwrap(); 
        let mut handle; 
        if let Some(p) = b.pop() {
            handle = p; 
        } else {
            eprintln!("\x1b[32;1m[Debug] Create a new buffer for packet data handling.\x1b[0m"); 
            handle = Box::new([0u8; BUFFER_SIZE]); 
        }
        drop(b);
        let r = udp_server.recv_from(handle.as_mut_slice());
        match r {
            Err(e) => { 
                match e.kind() {
                    // udp-server 被设置成非阻塞模式
                    // 在本框架中禁止这种行为
                    ErrorKind::WouldBlock => {
                        eprintln!("\x1b[31;1m[Error] UdpServer socket cann't be nonblocking.\x1b[0m");
                        eprintln!("\x1b[31;1m[Error] Attempt to reset the server as blocking. \x1b[0m"); 
                        udp_server.set_nonblocking(false).unwrap();
                    }
                    _ => {
                        unimplemented!("{e:?}")
                    },
                }
            },
            Ok((length, addr)) => {
                // 收取报文，并交付给线程池
                eprintln!("\x1b[32;1m[Debug] Receive a packet (id = {id}, byte num: {length}) from {addr}\x1b[0m"); 
                // 复制一个新的 UdpSocket 交给新的线程使用 
                let sender = udp_server.try_clone().unwrap(); 
                let s = move || {
                    let pid = id; 
                    let mut h = handle; 
                    if h.len() == length {
                        eprintln!("\x1b[33;1m[Warn ] Packet[id {pid}] may be discarded partially. \x1b[0m"); 
                    }
                    let _ = forward(&mut h[..length], pid, addr, sender); 
                    BUFFERS.lock().unwrap().push(h); 
                }; 
                id += 1; 
                task_manager.send(Box::new(s)).unwrap(); 
            },
        }
    }
}