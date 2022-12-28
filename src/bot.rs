use std::{
    collections::HashMap,
    str::FromStr,
    sync::{mpsc::channel, Arc, Mutex},
    thread::JoinHandle,
};

use regex::Regex;
use serde::Deserialize;

use crate::{
    action::Action,
    config::Config,
    server::{irc::Irc, shell::Shell, Server},
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerType {
    Shell,
    Irc,
}

type Callback = dyn Fn(&Bot, &str, &str, &str);

pub struct Message {
    pub channel: String,
    pub nick: String,
    pub message: String,
}

impl Message {
    pub fn trim(&self) -> &str {
        self.message.trim()
    }
}

pub struct Bot {
    name: String,
    reaction: HashMap<String, Box<Callback>>,
    resp: HashMap<String, Box<Callback>>,
    server: Arc<Mutex<Box<dyn Server>>>,
}

impl Bot {
    pub fn new(config: Config) -> Self {
        // https://rust-unofficial.github.io/patterns/idioms/on-stack-dyn-dispatch.html
        let server: Box<dyn Server> = match config.server {
            ServerType::Shell => Box::<Shell>::default(),
            ServerType::Irc => Box::new(Irc::new(
                config.irc.as_ref().expect("missing config").clone(),
            )),
        };

        Bot {
            name: config.name,
            reaction: HashMap::new(),
            resp: HashMap::new(),
            server: Arc::new(Mutex::new(server)),
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
        self.server.lock().unwrap().send(channel, message);
    }

    pub fn reply(&self, channel: &str, nick: &str, message: &str) {
        self.server
            .lock()
            .unwrap()
            .send(channel, &format!("{}: {}", nick, message));
    }

    pub fn run(&mut self) {
        self.install_actions();
        let server = self.server.clone();
        let (tx, rx) = channel::<Message>();
        let handle = server.lock().unwrap().connect(tx).unwrap();
        loop {
            let msg = rx.recv().unwrap();
            let message = msg.trim();

            if has_shutdown(&message.to_lowercase()) {
                self.shutdown();
                break;
            }

            // TODO 매번 regexp 를 compile 하지 않도록 해야 한다.
            for (pattern, cb) in &self.resp {
                let pat = format!("{}:? +?{}", self.name, pattern);
                let re = Regex::from_str(&pat).unwrap();
                if re.is_match(message) {
                    cb(self, &msg.channel, &msg.nick, message);
                }
            }

            for (pattern, cb) in &self.reaction {
                let re = Regex::from_str(pattern).unwrap();
                if re.is_match(message) {
                    cb(self, &msg.channel, &msg.nick, message);
                }
            }
        }

        self.finalize(handle);
    }

    pub fn shutdown(&mut self) {
        log::trace!("shutdown");
        self.server.lock().unwrap().disconnect();
    }

    pub fn finalize(&self, handle: JoinHandle<()>) {
        log::trace!("finalize...");
        handle.join().expect("join fail");
        log::trace!("finalize...done");
    }

    fn install_actions(&mut self) {
        // conditional install?
        self.hear("ping", &Action::ping);
    }
}

fn has_shutdown(s: &str) -> bool {
    matches!(s, "exit" | "quit" | "bye")
}
