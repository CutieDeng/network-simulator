use std::{sync::{mpsc::{channel, Sender}, Arc, Mutex}, thread::{spawn, JoinHandle}};

pub struct ThreadPool {
    _threads: Vec<JoinHandle<()>>,
}

impl ThreadPool {
    pub fn with_thread_num (thread_size: usize) -> (Self, Sender<Box<dyn FnOnce() + Send>>) {
        let (send, recv): (Sender<Box<dyn FnOnce() + Send>>, _) = channel();
        let recv = Arc::new(Mutex::new(recv)); 
        let mut threads = Vec::new(); 
        for _ in 0..thread_size {
            let recv = recv.clone(); 
            let s = spawn(move || {
                let r = recv; 
                loop {
                    let r = r.lock().unwrap(); 
                    let rec = r.recv(); 
                    drop(r); 
                    match rec {
                        Ok(rec) => {
                            rec(); 
                        },
                        Err(_) => {
                            break 
                        },
                    }
                }
            }); 
            threads.push(s);
        }
        ( Self { _threads: threads }, send )
    }
}

pub mod mysocket; 