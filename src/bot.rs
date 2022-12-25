use std::{
    collections::HashMap,
    str::FromStr,
    sync::mpsc::{channel, Sender},
    thread::JoinHandle,
};

use regex::Regex;

use crate::{
    action::Action,
    server::{echo::Echo, irc::Irc, shell::Shell, Server},
};

pub enum ServerType {
    Shell,
    Irc,
    Echo,
}

type Callback = dyn Fn(&Bot, &str, &str, &str);

pub struct Bot {
    name: String,
    server_type: ServerType,
    reaction: HashMap<String, Box<Callback>>,
    resp: HashMap<String, Box<Callback>>,
    running: bool,
    sender: Option<Sender<(String, String, String)>>,
}

impl Bot {
    pub fn new(name: &str, server_type: ServerType) -> Self {
        Bot {
            name: name.to_string(),
            server_type,
            running: false,
            reaction: HashMap::new(),
            resp: HashMap::new(),
            sender: None,
        }
    }

    pub fn hear(&mut self, pattern: &str, cb: &'static Callback) {
        self.reaction
            .entry(pattern.to_string())
            .or_insert_with(|| Box::new(cb));
    }

    pub fn respond(&mut self, pattern: &str, cb: &'static Callback) {
        self.resp
            .entry(pattern.to_string())
            .or_insert_with(|| Box::new(cb));
    }

    pub fn send(&self, channel: &str, message: &str) {
        if let Some(tx) = &self.sender {
            tx.send((
                channel.to_string(),
                self.name.to_string(),
                message.to_string(),
            ))
            .expect("send fail");
        }
    }

    pub fn reply(&self, channel: &str, nick: &str, message: &str) {
        if let Some(tx) = &self.sender {
            tx.send((
                channel.to_string(),
                self.name.to_string(),
                format!("{}: {}", nick, message),
            ))
            .expect("send fail");
        }
    }

    pub fn run(&mut self) {
        self.running = true;
        self.install_actions();

        // https://rust-unofficial.github.io/patterns/idioms/on-stack-dyn-dispatch.html
        let mut server: Box<dyn Server> = match &self.server_type {
            ServerType::Shell => Box::<Shell>::default(),
            ServerType::Irc => Box::<Irc>::default(),
            ServerType::Echo => Box::<Echo>::default(),
        };

        let (tx, rx) = channel::<(String, String, String)>();
        self.sender = Some(tx.clone());
        let mut handlers = vec![server.connect(tx).unwrap()];

        while self.running {
            let (channel, nick, got) = rx.recv().unwrap();
            let message = got.trim();

            if nick.ne("you") {
                println!("{:>7}#{}> {}", nick, channel, message);
            }

            if has_shutdown(message) {
                server.disconnect();
                self.shutdown();
            }

            for (pattern, cb) in &self.resp {
                let pat = format!("{}:? +?{}", self.name, pattern);
                let re = Regex::from_str(&pat).unwrap();
                if re.is_match(message) {
                    cb(self, &channel, &nick, message);
                }
            }

            for (pattern, cb) in &self.reaction {
                let re = Regex::from_str(pattern).unwrap();
                if re.is_match(message) {
                    cb(self, &channel, &nick, message);
                }
            }
        }

        self.finalize(&mut handlers);
    }

    pub fn shutdown(&mut self) {
        log::trace!("shutdown");
        self.running = false;
    }

    pub fn finalize(&self, handlers: &mut Vec<JoinHandle<()>>) {
        log::trace!("finalize...");
        while let Some(h) = handlers.pop() {
            h.join().expect("couldn't join thread");
        }
        log::trace!("finalize...done");
    }

    fn install_actions(&mut self) {
        self.hear("ping", &Action::ping);
    }
}

fn has_shutdown(s: &str) -> bool {
    matches!(s, "exit" | "quit" | "bye")
}
