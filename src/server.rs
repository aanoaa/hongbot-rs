use std::{
    io,
    sync::{mpsc::Sender, Arc, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

pub trait Server {
    fn connect(&mut self, tx: Sender<String>) -> Result<JoinHandle<()>, &str>;
    fn disconnect(&self);
}

#[derive(Debug)]
pub struct Empty {
    accepted: Option<Arc<RwLock<bool>>>,
}

impl Empty {
    pub fn new() -> Self {
        Empty { accepted: None }
    }
}

impl Default for Empty {
    fn default() -> Self {
        Self::new()
    }
}

impl Server for Empty {
    fn connect(&mut self, tx: Sender<String>) -> Result<JoinHandle<()>, &str> {
        log::trace!("connect");
        let lock0 = Arc::new(RwLock::new(true));
        let lock1 = Arc::clone(&lock0);
        self.accepted = Some(lock0);
        let handle = thread::spawn(move || {
            let stdin = io::stdin();
            let dur = Duration::from_millis(10);
            while *(lock1.read().expect("acquire read lock fail")) {
                let mut buf = String::new();
                stdin.read_line(&mut buf).expect("read fail");
                let message = buf.trim();
                tx.send(message.to_string()).expect("send fail");
                // sleep 을 주지 않으면 disconnect 에 의해 accepted 값이
                // 변경되기 전에 loop 로 들어와서 표준입력을 기다림 -> 뭐라도 눌러야 종료되는 상황
                thread::sleep(dur);
            }
        });

        Ok(handle)
    }

    fn disconnect(&self) {
        log::trace!("disconnect");
        if let Some(lock) = &self.accepted {
            let mut lock = lock.write().expect("acquire write lock fail");
            *lock = false;
        }
    }
}
