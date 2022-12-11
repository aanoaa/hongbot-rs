use std::{
    io::{self, stdout, Result, Write},
    sync::{mpsc::Sender, Arc, RwLock},
    thread,
    time::Duration,
};

pub trait Server {
    fn connect(&mut self);
    fn disconnect(&mut self);
    fn on_message(&mut self) -> Result<()>;
    fn on_connect(&mut self);
    fn on_close(&self);
}

pub struct Shell {
    pub tx: Sender<String>,
    pub shutdown: Arc<RwLock<bool>>,
}

impl Shell {
    pub fn new(tx: Sender<String>, shutdown: Arc<RwLock<bool>>) -> Self {
        Shell { tx, shutdown }
    }
}

impl Server for Shell {
    fn connect(&mut self) {
        log::trace!("connect");
        self.on_connect();
    }

    fn disconnect(&mut self) {
        log::trace!("disconnect");
        self.on_close();
    }

    // stream 이라하고,
    fn on_message(&mut self) -> Result<()> {
        log::trace!("on_message");
        let stdin = io::stdin();
        let mut stdout = stdout();
        let dur = Duration::from_millis(10);
        loop {
            {
                let shutdown = self.shutdown.read().unwrap();
                if *shutdown {
                    break;
                }
            }

            let mut buf = String::new();
            print!("you> ");
            stdout.flush()?;
            stdin.read_line(&mut buf)?;
            let message = buf.trim();
            self.tx.send(message.to_string()).expect("send fail");
            thread::sleep(dur);
        }

        self.disconnect();
        Ok(())
    }

    fn on_connect(&mut self) {
        log::trace!("on_connect");
        self.on_message().ok();
    }

    fn on_close(&self) {
        log::trace!("on_close");
    }
}
