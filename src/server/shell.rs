use std::{
    io::{self, stdout, Write},
    sync::{mpsc::Sender, Arc, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Result;

use crate::bot::Message;

use super::Server;

#[derive(Debug)]
pub struct Shell {
    tx: Option<Sender<Message>>,
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

const SHELL_SERVER_CHANNEL: &str = "#shell";
const SHELL_SERVER_NICK: &str = "you";
const SHELL_SERVER_BOT_NAME: &str = "bot";

impl Server for Shell {
    fn connect(&mut self, tx: Sender<Message>) -> Result<JoinHandle<()>> {
        log::trace!("connect");
        let lock0 = Arc::new(RwLock::new(true));
        let lock1 = Arc::clone(&lock0);
        self.accepted = Some(lock0);
        self.tx = Some(tx.clone());
        let handle = thread::spawn(move || {
            let stdin = io::stdin();
            let dur = Duration::from_millis(10);
            let mut stdout = stdout();
            let mut buf = String::new();
            while *(lock1.read().expect("acquire read lock fail")) {
                print!("{}{}> ", SHELL_SERVER_NICK, SHELL_SERVER_CHANNEL);
                stdout.flush().unwrap();
                stdin.read_line(&mut buf).expect("read fail");
                tx.send(Message {
                    channel: SHELL_SERVER_CHANNEL.to_string(),
                    nick: SHELL_SERVER_NICK.to_string(),
                    message: buf.clone(),
                })
                .expect("send fail");
                buf.clear();

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

    fn send(&mut self, channel: &str, message: &str) {
        println!("{}{}> {}", SHELL_SERVER_BOT_NAME, channel, message);
    }
}
