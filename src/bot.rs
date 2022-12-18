use std::{
    io::{stdout, Write},
    sync::mpsc::channel,
    thread::JoinHandle,
};

use crate::server::{Empty, Server};

pub struct Bot {
    pub name: String,
    // server params...
    running: bool,
}

// trait 으로 interface 명확하게 하자
impl Bot {
    pub fn new(name: &str) -> Self {
        Bot {
            name: name.to_string(),
            running: false,
        }
    }

    pub fn run(&mut self) {
        self.running = true;

        // https://rust-unofficial.github.io/patterns/idioms/on-stack-dyn-dispatch.html
        let mut server: Box<dyn Server> = match &self.server {
            ServerType::Empty => Box::<Empty>::default(),
            ServerType::Shell => Box::<Shell>::default(),
        };

        let (tx, rx) = channel::<String>();
        let j_handles = vec![server.connect(tx).unwrap()];

        let mut stdout = stdout();
        while self.running {
            print!("you> ");
            stdout.flush().unwrap();
            let got = rx.recv().unwrap();
            let message = got.trim();
            if has_shutdown(message) {
                server.disconnect();
                self.shutdown();
            }
        }
        self.finalize(j_handles);
    }

    pub fn shutdown(&mut self) {
        log::trace!("shutdown");
        self.running = false;
    }

    pub fn finalize(&self, handles: Vec<JoinHandle<()>>) {
        log::trace!("finalize...");
        for h in handles {
            h.join().expect("couldn't join thread");
        }
        log::trace!("finalize...done");
    }
}

fn has_shutdown(s: &str) -> bool {
    matches!(s, "exit" | "quit" | "bye")
}
