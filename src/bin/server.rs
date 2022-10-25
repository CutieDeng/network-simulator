use std::{sync::{Arc, Mutex, atomic::Ordering::Relaxed}, net::{SocketAddr, IpAddr, Ipv4Addr}, collections::BTreeMap, time::Duration};

use tokio::{runtime::Handle, net::UdpSocket, time::sleep};

use lazy_static::lazy_static; 

const BUFFER_LENGTH: usize = 2048 + 6; 
type BufferType = [u8; BUFFER_LENGTH]; 

static BUFFER_QUEUE : Mutex<Vec<Box<BufferType>>> = Mutex::new(Vec::new()); 

mod packets {
    use std::sync::atomic::AtomicUsize;
    /// 记录服务器一段时间内接受的包的数目
    pub static RECEIVE_NUMBER : AtomicUsize = AtomicUsize::new(0); 
    /// 记录服务器一段时间内转发的包的数目
    pub static _SEND_NUMBER : AtomicUsize = AtomicUsize::new(0); 
    // 同时计算丢包率
}

lazy_static! {
    static ref LINKS_BITWIDTH : Mutex<BTreeMap<(SocketAddr, SocketAddr), usize>> = Mutex::new(BTreeMap::new()); 
}

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

fn _load_config () {
    const SERVER_IP : &str = "127.67.117.116:52736"; 
}

async fn exec(rt: &Handle) {
    let core_socket = UdpSocket::bind("127.67.117.116:52736").await.unwrap();
    eprintln!("\x1b[36;1m[INFO ] 服务器启动，udp 地址：{}\x1b[0m", core_socket.local_addr().unwrap()); 
    let core_socket = Arc::new(core_socket); 
    let mut index : usize = 0; 
    {
        rt.spawn(async {
            loop {
                let packets_number = packets::RECEIVE_NUMBER.swap(0, Relaxed); 
                eprintln!("\x1b[32;1m[DEBUG] 服务器本周期接受了 {packets_number} 个包\x1b[0m"); 
                sleep(Duration::from_secs(2)).await; 
            }
        }); 
    }
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
        packets::RECEIVE_NUMBER.fetch_add(1, Relaxed); 
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
    let p = {
        LINKS_BITWIDTH.lock().unwrap().get(&(src, target)).map(|f| *f) 
    }; 
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