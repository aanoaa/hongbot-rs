use std::{
    io::{self, stdout, Write},
    sync::{mpsc::Sender, Arc, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

pub trait Server {
    fn connect(&mut self, tx: Sender<(String, String, String)>) -> Result<JoinHandle<()>, &str>;
    fn disconnect(&self);
}

#[derive(Debug)]
pub struct Shell {
    tx: Option<Sender<(String, String, String)>>,
    accepted: Option<Arc<RwLock<bool>>>,
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            accepted: None,
            tx: None,
        }
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Server for Shell {
    fn connect(&mut self, tx: Sender<(String, String, String)>) -> Result<JoinHandle<()>, &str> {
        log::trace!("connect");
        let lock0 = Arc::new(RwLock::new(true));
        let lock1 = Arc::clone(&lock0);
        self.accepted = Some(lock0);
        self.tx = Some(tx.clone());
        let handle = thread::spawn(move || {
            let stdin = io::stdin();
            let dur = Duration::from_millis(10);
            let mut stdout = stdout();
            while *(lock1.read().expect("acquire read lock fail")) {
                let mut buf = String::new();
                print!("you> ");
                stdout.flush().unwrap();
                stdin.read_line(&mut buf).expect("read fail");
                let channel = String::from("empty");
                let nick = String::from("you");
                let message = buf.trim();
                tx.send((channel, nick, message.to_string()))
                    .expect("send fail");
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
