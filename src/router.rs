use std::{sync::{Arc, atomic::{AtomicUsize, Ordering::Relaxed}}, collections::{BTreeMap, LinkedList}, net::{Ipv4Addr, SocketAddrV4}, time::Duration};

use tokio::{sync::{Mutex, mpsc::{self, UnboundedReceiver, UnboundedSender, error::TryRecvError}}, task::yield_now, time::{Instant, sleep}, net::UdpSocket, spawn};
use lazy_static::lazy_static;

use crate::router::config::drop_packet; 

#[derive(Debug)]
pub struct Message {
    pub target: SocketAddrV4,
    pub message: MessageType, 
    pub message_len: usize, 
}

pub const MESSAGE_LENGTH : usize = 2500; 
pub type MessageType = Box<[u8; MESSAGE_LENGTH]>; 

pub struct Router {
    ipv4addr: Ipv4Addr, 
    outers: Mutex<BTreeMap<Ipv4Addr, (usize, UnboundedSender<Message>)>>, 
    receiver: Mutex<UnboundedReceiver<Message>>, 
    sender: UnboundedSender<Message>, 
    pub queue_size: AtomicUsize, 
    routers: Mutex<BTreeMap<Ipv4Addr, (f64, Ipv4Addr)>>, 
}

const DEFAULT_QUEUE_SIZE: AtomicUsize = AtomicUsize::new(5);

const PERIOD_UPDATE: Duration = Duration::from_secs(20); 

lazy_static! {
    pub static ref GLOBAL_ROUTERS: Mutex<BTreeMap<Ipv4Addr, Arc<Router>>> = Mutex::const_new(BTreeMap::new()); 
    pub static ref CACHES: Mutex<LinkedList<MessageType>> = Mutex::new(LinkedList::new()); 
    pub static ref GLOBAL_MAPS: Mutex<()> = Mutex::new(()); 
}

pub static SERVER_SOCKET: Option<UdpSocket> = None; 

impl Router {

    pub const fn outers(&self) -> &Mutex<BTreeMap<Ipv4Addr, (usize, UnboundedSender<Message>)>> {
        &self.outers
    }

    pub const fn sender(&self) -> &UnboundedSender<Message> {
        &self.sender
    }

    pub const fn ipv4addr(&self) -> Ipv4Addr {
        self.ipv4addr
    }

    pub async fn from_ipv4addr(ipv4: Ipv4Addr, udp: Arc<UdpSocket>) -> Arc<Router> {
        let mut guard = GLOBAL_ROUTERS.lock().await; 
        let entry = guard.entry(ipv4);
        let mut created = false; 
        let value = entry.or_insert_with( || {
            created = true; 
            let (s, r) = mpsc::unbounded_channel(); 
            Arc::new(Router {
                ipv4addr: ipv4, 
                outers: Mutex::new(BTreeMap::new()), 
                receiver: Mutex::new(r), 
                sender: s, 
                queue_size: DEFAULT_QUEUE_SIZE, 
                routers: Mutex::new(BTreeMap::new()), 
            }) 
        }); 
        if created {
            let value = value.clone(); 
            if cfg!(feature = "log-deal") {
                eprintln!("\x1b[32;1m[{:21}] ip: {}\x1b[0m", "Router Create", value.ipv4addr); 
            }
            spawn(async move {
                let t = udp; 
                value.work(&t).await; 
            }); 
        }
        value.clone()
    }

    pub async fn work(&self, sender: &UdpSocket) {
        let mut queue = LinkedList::new();
        let mut to_send: Option<(Message, f64)> = None; 
        let mut last_instant = Instant::now(); 
        loop {
            let mut receiver = self.receiver.lock().await; 
            'recv: loop {
                match receiver.try_recv() { 
                    Ok(r) => {
                        if queue.len() < self.queue_size.load(Relaxed) {
                            queue.push_back(r); 
                        } else {
                            let p = if cfg!(feature = "log-drop") {
                                format!("queue buffer overflow; router: {}", self.ipv4addr)
                            } else { 
                                "".to_string() 
                            }; 
                            drop_packet(r.message_len, &p, r.message).await; 
                        }
                    },
                    Err(TryRecvError::Empty) => {
                        break 'recv; 
                    },
                    _ => unreachable!(), 
                }
            };
            drop(receiver); 
            if let None = to_send {
                // move a new packet from queue to it. 
                if let Some(m) = queue.pop_front() {
                    let ml = m.message_len; 
                    to_send = Some((m, ((ml + 2) * 8) as f64)); 
                }
            }
            if let Some((ref i, ref mut val)) = to_send {
                if *i.target.ip() == self.ipv4addr {
                    // send the packet to the actual position! 
                    sender.send_to(&i.message[..i.message_len], i.target).await.unwrap(); 
                    CACHES.lock().await.push_back(to_send.unwrap().0.message); 
                    to_send = None; 
                } else {
                    sleep(Duration::from_millis(100)).await; 
                    let router = self.routers.lock().await; 
                    let target = router.get(i.target.ip()).map(|v| v.1);
                    drop(router); 
                    match target {
                        Some(p) => {
                            let sender = self.outers.lock().await.get(&p).map(|a| (a.0, a.1.clone())); 
                            match sender {
                                Some((bw, send)) => {
                                    let bw = bw as f64 / 10.; 
                                    *val -= bw; 
                                    if *val <= 0. {
                                        send.send(to_send.unwrap().0).unwrap(); 
                                        to_send = None; 
                                    } 
                                },
                                None => {
                                    let hint = if cfg!(feature = "log-drop") {
                                        format!("impossible miss router op; locate router: {}", self.ipv4addr) 
                                    } else { "".to_string() }; 
                                    drop_packet(i.message_len, &hint, to_send.unwrap().0.message).await; 
                                    to_send = None; 
                                },
                            }
                        },
                        None => {
                            let hint = 
                                if cfg!(feature = "log-drop") {
                                    format!("packet (target {}:{:5}) fails with the missing routing item; router: {}", i.target.ip(), i.target.port(), self.ipv4addr)
                                } else {
                                    "".into()
                                }; 
                            drop_packet(i.message_len, &hint, to_send.unwrap().0.message).await; 
                            to_send = None; 
                        },
                    }
                }
            }
            let now = Instant::now(); 
            if now - last_instant > PERIOD_UPDATE {
                let origin_items; 
                let globals = GLOBAL_ROUTERS.lock().await; 
                let mut routers = self.routers.lock().await; 
                origin_items = routers.len(); 
                routers.clear(); 
                {
                    let outer = self.outers.lock().await; 
                    for (t, (bw, _)) in outer.iter() {
                        let entry = routers.entry(*t);
                        if *bw == 0 {
                            continue 
                        }
                        let speed = 1. / *bw as f64; 
                        entry.and_modify(|v| {
                            if v.0 < speed {
                                *v = (speed, *t); 
                            }
                        }).or_insert((speed, *t)); 
                    }
                }
                let p: Vec<_> = self.outers.lock().await.iter().map(|(ipv4, (bw, _))| (*ipv4, *bw)).collect(); 
                for (ip, bw) in p {
                    let speed = 1. / bw as f64; 
                    if bw == 0 { continue }
                    let g3 = globals.get(&ip); 
                    match g3 {
                        Some(g3) => {
                            let r2 = g3.routers.lock().await;
                            for (target, (sp2, _)) in r2.iter() {
                                if *target == self.ipv4addr { continue }
                                let entry = routers.entry(*target); 
                                let speed = speed + sp2; 
                                entry.and_modify(|v| {
                                    if v.0 < speed {
                                        *v = (speed, g3.ipv4addr); 
                                    }
                                }).or_insert((speed, g3.ipv4addr)); 
                            }
                        },
                        None => {},
                    }
                }
                drop(globals); 
                // calculate the end... 
                let new_item_len = routers.len(); 
                drop(routers); 
                if cfg!(feature = "log-update") {
                    eprintln!("\x1b[32;1m[{:21}] {}\x1b[0m", "Router Table Update", 
                        if origin_items == new_item_len {
                            format!("table size({origin_items}) not changed. ")
                        } else {
                            format!("table size {} -> {}. ", origin_items, new_item_len)
                        })
                }
                // update your last update time! 
                last_instant = now; 
            }
            yield_now().await; 
        }; 
    }
}

pub mod config {
    
    use std::sync::atomic::AtomicUsize;

    use std::sync::atomic::Ordering::Relaxed;

    use super::{MessageType, CACHES}; 

    pub static LOSS_PACKETS: AtomicUsize = AtomicUsize::new(0); 
    pub static RECEIVE_PACKETS: AtomicUsize = AtomicUsize::new(0); 

    pub static LOSS_BYTES: AtomicUsize = AtomicUsize::new(0); 
    pub static RECEIVE_BYTES: AtomicUsize = AtomicUsize::new(0); 

    pub async fn drop_packet(input: usize, hint: &str, packet: MessageType) {
        if input < 6 {
            // impossible, without the proper bytes ahead... 
            eprintln!("\x1b[31;1m[{:21}] cause: {hint}\x1b[0m", "Packet Length Invalid");
        } else {
            LOSS_PACKETS.fetch_add(1, Relaxed); 
            LOSS_BYTES.fetch_add(input - 6, Relaxed); 
            if cfg!(feature = "log-drop") {
                eprintln!("\x1b[32;1m[{:21}] cause: {hint}\x1b[0m", "Drop Event at Route");
            }
        }
        CACHES.lock().await.push_back(packet); 
    }
}