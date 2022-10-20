use std::{sync::{Arc, Mutex}, net::{SocketAddr, IpAddr, Ipv4Addr}, collections::BTreeMap, time::Duration};

use tokio::{runtime::Handle, net::UdpSocket, time::sleep};

use lazy_static::lazy_static; 

const BUFFER_LENGTH: usize = 2048 + 6; 
type BufferType = [u8; BUFFER_LENGTH]; 

static BUFFER_QUEUE : Mutex<Vec<Box<BufferType>>> = Mutex::new(Vec::new()); 

lazy_static! {
    static ref LINKS_BITWIDTH : Mutex<BTreeMap<(SocketAddr, SocketAddr), usize>> = Mutex::new(BTreeMap::new()); 
}

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(7)
        .enable_all()
        .build()
        .unwrap();
    eprintln!("\x1b[32;1m[DEBUG] tokio runtime 构造完成\x1b[0m"); 
    rt.block_on(exec(rt.handle())); 
}

async fn exec(rt: &Handle) {
    let core_socket = UdpSocket::bind("localhost:23333").await.unwrap();
    eprintln!("\x1b[36;1m[INFO ] 服务器启动，udp 地址：{}\x1b[0m", core_socket.local_addr().unwrap()); 
    let core_socket = Arc::new(core_socket); 
    let mut index : usize = 0; 
    loop {
        let mut buffer; 
        let mut bq = BUFFER_QUEUE.lock().unwrap(); 
        buffer = if let Some(buf) = bq.pop() {
            buf
        } else {
            Box::new([0u8; BUFFER_LENGTH])
        }; 
        drop(bq); 
        let (length, src) = core_socket.recv_from(buffer.as_mut_slice()).await.unwrap(); 
        if length == BUFFER_LENGTH {
            eprintln!("\x1b[33;1m[WARN ] 收取到数据报文 [id={index} src={src}], 报文内容过长，被自动丢弃。\x1b[0m");
            index += 1; 
            continue 
        }
        let sender = Arc::clone(&core_socket); 
        let p = async move {
            send(sender, &mut buffer[..length], src, index).await; 
            let mut bq = BUFFER_QUEUE.lock().unwrap(); 
            bq.push(buffer); 
            drop(bq); 
        }; 
        index += 1; 
        rt.spawn(p); 
    }
}

async fn send(sender: Arc<UdpSocket>, buffer: &mut [u8], src: SocketAddr, pid: usize) {
    if buffer.len() < 6 {
        eprintln!("\x1b[31;1m[WARN ] 报文 (ID={pid}) 长度过短，无法解析其接下来的目标地址，被自动丢弃\x1b[0m");
        return 
    }
    let target = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(buffer[0], buffer[1], buffer[2], buffer[3])), 
        ( buffer[5] as u16 ) << 8 | buffer[4] as u16 ); 
    eprintln!("\x1b[32;1m[DEBUG] Start forward packet[id {pid}]. \x1b[0m",);
    match src {
        SocketAddr::V4(v4) => {
            for i in 0..4 {
                buffer[i] = v4.ip().octets()[i]; 
            }
            buffer[4] = v4.port() as u8; 
            buffer[5] = (v4.port() >> 8) as u8;  
            if buffer[0] != 127 {
                eprintln!("\x1b[33;1m[WARN ] 报文 (ID={pid}) 来源地址未知 {{{v4}}}，报文被丢弃\x1b[0m"); 
            }
        }
        SocketAddr::V6(v6) => {
            eprintln!("\x1b[33;1m[WARN ] 报文 (ID={pid}) 来自 ipv6 {v6}, 被自动丢弃 \x1b[0m"); 
            return 
        }
    }
    let p; 
    {
        let link_bw = LINKS_BITWIDTH.lock().unwrap(); 
        p = link_bw.get(&(src, target)).map(|f| *f); 
        drop(link_bw); 
    }
    match p {
        Some(bw) => {
            let time_cost = (buffer.len() + 6) * 8; 
            if bw == 0 {
                eprintln!("\x1b[33;1m[Error] 报文 (ID={pid}) 所在链路的位宽为零！\x1b[0m"); 
                return 
            }
            let time_cost = time_cost / bw + 1; 
            sleep(Duration::from_millis(time_cost as u64)).await; 
        },
        None => {
            eprintln!("\x1b[33;1m[WARN ] 报文 (ID={pid}) 不在一条合法的链路上传输，{src} ~ {target} 之间没有通路\x1b[0m"); 
            return 
        },
    }
    eprintln!("\x1b[36;1m[INFO ] 报文 (ID={pid}) 从 {src} 发送至 {target} 完成\x1b[0m"); 
    let d = sender.send_to(&buffer, target).await; 
    let d = d.unwrap(); 
    if d != buffer.len() {
        eprintln!("\x1b[31;1m[Error] 报文 (ID={pid}) 只发送了 {d} 字节，期望发送 {} 字节。\x1b[0m", buffer.len())
    }
}