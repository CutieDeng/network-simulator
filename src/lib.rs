use std::{borrow::Cow, collections::BTreeMap, net::SocketAddr, time::Duration, sync::atomic::AtomicUsize};

use lazy_static::lazy_static;
use tokio::{sync::{RwLock, Mutex}, task::{yield_now, JoinHandle}, time::Instant, runtime::Handle};

use std::sync::atomic::Ordering::Relaxed;

pub struct ServerConfig <'a, 'b> {
    local_addr: Cow<'a, str>, 
    controller_addr: Option<Cow<'b, str>>,
}

lazy_static! {
    pub static ref GLOBAL_MAP : RwLock<BTreeMap<SocketAddr, BTreeMap<SocketAddr, usize>>> = RwLock::new(BTreeMap::new()); 
    pub static ref ASYNC_MAP: Mutex<BTreeMap<SocketAddr, JoinHandle<()>>> = Mutex::new(BTreeMap::new()); 
}

static EVENT_UID: AtomicUsize = AtomicUsize::new(0); 

pub const LONG_TIME: Duration = Duration::from_secs(60); 
pub const SHORT_TIME: Duration = Duration::from_secs(3); 

// pub async fn forward(rt: &Handle, c

/// 阻塞式 create & reset link 调用（执行阻塞，但函数非阻塞）
/// 
/// 该调用会创建一个新的阻塞线程完成工作
pub fn create_or_reset_link_blocking(rt: &Handle, src: SocketAddr, dst: SocketAddr, bitwidth: usize) {
    rt.spawn_blocking(move || {
        let event_uid = EVENT_UID.fetch_add(1, Relaxed); 
        eprintln!("\x1b[32;1m[DEBUG] 链路修改事件 (uid={event_uid}) {src} -> {dst} bw: {bitwidth} 开始阻塞\x1b[0m"); 
        let mut gm = GLOBAL_MAP.blocking_write();
        gm.entry(src).or_default().entry(dst).and_modify(|val| *val = bitwidth).or_insert(bitwidth); 
        drop(gm); 
        eprintln!("\x1b[32;1m[DEBUG] 链路修改事件 (uid={event_uid}) 完成\x1b[0m"); 
    }); 
}

pub fn active_socket_addr(rt: &Handle, node: SocketAddr) {
    let copy_rt = rt.clone(); 
    rt.spawn(async move {
        let event_uid = EVENT_UID.fetch_add(1, Relaxed); 
        eprintln!("\x1b[32;1m[DEBUG] uid={event_uid} 启动节点 {node} 转发服务"); 
        let mut async_map = ASYNC_MAP.lock().await; 
        let entry = async_map.entry(node); 
        let mut exist = true; 
        entry.or_insert_with(|| {
            exist = false; 
            copy_rt.spawn(async move {
                // emmmm 
            })
        }); 
        drop(async_map); 
        if exist {
            eprintln!("\x1b[33;1m[WARN ] uid={event_uid} 尝试重复启动节点 {} 的网络服务\x1b[0m", node);
        } else {
            eprintln!("\x1b[32;1m[DEBUG] uid={event_uid} 成功启动 {node} 网络服务\x1b[0m"); 
        }
    });
}

pub fn unactive_socket_addr(rt: &Handle, node: SocketAddr) {
    rt.spawn(async move {
        let event_uid = EVENT_UID.fetch_add(1, Relaxed); 
        eprintln!("\x1b[32;1m[DEBUG] uid={event_uid} 关闭节点 {node} 转发服务"); 
        let mut async_map = ASYNC_MAP.lock().await; 
        let result = async_map.remove(&node); 
        drop(async_map); 
        match result {
            Some(jo) => {
                jo.abort(); 
                eprintln!("\x1b[32;1m[DEBUG] uid={event_uid} 成功关闭 {node} 网络服务\x1b[0m"); 
            },
            None => {
                eprintln!("\x1b[33;1m[WARN ] uid={event_uid} 节点 {} 服务没有开启\x1b[0m", node);
            },
        } 
    });
}

/// 非阻塞式 create & reset link 调用 
/// 
/// 该调用需要一个 timeout 来描述其超时的时间，这意味着它不能保证该事件被完成
/// 该调用依旧会构建一个新的协程进行该操作的处理
pub fn create_or_reset_link_async(rt: &Handle, src: SocketAddr, dst: SocketAddr, bitwidth: usize, timeout: Duration) {
    rt.spawn( async move {
        let start = Instant::now(); 
        let event_uid = EVENT_UID.fetch_add(1, Relaxed); 
        eprintln!("\x1b[32;1m[DEBUG] 链路修改事件 (uid={event_uid}) {src} -> {dst} bw: {bitwidth}\x1b[0m");
        loop {
            match GLOBAL_MAP.try_write() {
                Ok(mut global_map) => {
                    let mut e = "新增";  
                    global_map.entry(src).or_default().entry(dst)
                        .and_modify(|val| { e = "修改"; *val = bitwidth }).or_insert(bitwidth); 
                    eprintln!("\x1b[32;1m[DEBUG] 链路{} (uid={event_uid}) 完成\x1b[0m", e); 
                },
                Err(_) => {
                    let now = Instant::now(); 
                    let tcost = now - start; 
                    if tcost >= timeout {
                        eprintln!("\x1b[33;1m[WARN ] 链路修改 (uid={event_uid}) 超时放弃\x1b[0m");
                        break 
                    } else {
                        yield_now().await; 
                    }
                },
            }
        }
    } ); 
}

impl <'a, 'b> ServerConfig<'a, 'b> {
    pub fn local_addr(&self) -> &str {
        &self.local_addr
    }
    pub fn controller_addr(&self) -> Option<&str> {
        if let Some(s) = &self.controller_addr {
            Some(s)
        } else {
            None
        }
    }
}

pub mod router; 