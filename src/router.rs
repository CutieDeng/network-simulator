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
    queue_size: AtomicUsize, 
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
            spawn(async move {
                let t = udp; 
                value.work(&t).await; 
            }); 
        }
        value.clone()
    }

    pub async fn work(&self, sender: &UdpSocket) {
        let mut queue = LinkedList::new();
        let mut last_instant = Instant::now(); 
        loop {
            let mut receiver = self.receiver.lock().await; 
            'recv: loop {
                match receiver.try_recv() { 
                    Ok(r) => {
                        if queue.len() < self.queue_size.load(Relaxed) {
                            queue.push_back(r); 
                        } else {
                            // drop the value r. 
                            CACHES.lock().await.push_back(r.message);
                            // remember the information... 
                        }
                    },
                    Err(TryRecvError::Empty) => {
                        break 'recv; 
                    },
                    _ => unreachable!(), 
                }
            };
            drop(receiver); 
            if let Some(m) = queue.pop_front() {
                if *m.target.ip() == self.ipv4addr {
                    sender.send_to(&m.message[..m.message_len], m.target).await.unwrap(); 
                    CACHES.lock().await.push_back(m.message); 
                } else {
                    let router = self.routers.lock().await; 
                    let target = router.get(m.target.ip()).map(|v| v.1);
                    drop(router); 
                    match target {
                        Some(p) => {
                            let sender = self.outers.lock().await.get(&p).map(|a| (a.0, a.1.clone())); 
                            match sender {
                                Some((bw, send)) => {
                                    let packet_bytes = m.message_len + 2; 
                                    let packet_bits = ( packet_bytes * 8 ) as f64;  
                                    let cost_time = packet_bits / bw as f64; 
                                    let cost_time_cal = Duration::from_secs_f64(cost_time); 
                                    if cost_time_cal >= PERIOD_UPDATE {
                                        drop_packet(m.message_len, "packet route waiting would timeout", m.message).await; 
                                    } else {
                                        sleep(cost_time_cal).await; 
                                        send.send(m).unwrap(); 
                                    }
                                },
                                None => {
                                    drop_packet(m.message_len, "packet route to the impossible direction", m.message).await; 
                                },
                            }
                        },
                        None => {
                            let hint = format!("packet (target {}:{:5}) fails with the missing routing item (router {})", m.target.ip(), m.target.port(), self.ipv4addr); 
                            drop_packet(m.message_len, &hint, m.message).await; 
                        },
                    }; 
                }
            }
            let now = Instant::now(); 
            if now - last_instant > PERIOD_UPDATE {
                // do something... 
                // let single_update = GLOBAL_MAPS.lock().await; 
                let origin_items; 
                let globals = GLOBAL_ROUTERS.lock().await; 
                let mut routers = self.routers.lock().await; 
                origin_items = routers.len(); 
                routers.clear(); 
                {
                    let outer = self.outers.lock().await; 
                    for (t, (bw, _)) in outer.iter() {
                        let entry = routers.entry(*t);
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
                    let g3 = globals.get(&ip); 
                    match g3 {
                        Some(g3) => {
                            let r2 = g3.routers.lock().await;
                            for (target, (sp2, _)) in r2.iter() {
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
                eprintln!("\x1b[32;1[DEBUG] router {}: update router table, size {} -> {}. \x1b[0m", self.ipv4addr, 
                    origin_items, new_item_len); 
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

    pub(super) async fn drop_packet(input: usize, hint: &str, packet: MessageType) {
        if input < 6 {
            // impossible, without the proper bytes ahead... 
            eprintln!("\x1b[31;1m[ERROR] Packet Invalid: {hint}\x1b[0m");
        } else {
            LOSS_PACKETS.fetch_add(1, Relaxed); 
            LOSS_BYTES.fetch_add(input - 6, Relaxed); 
            eprintln!("\x1b[32;1m[DEBUG] Packet Drop: {hint}\x1b[0m");
        }
        CACHES.lock().await.push_back(packet); 
    }
}