use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
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

// Regex does not impl PartialEq, Eq, Hash trait
struct MyRegex(regex::Regex);

impl PartialEq for MyRegex {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str().eq(other.0.as_str())
    }
}
impl Eq for MyRegex {}

impl MyRegex {
    fn from_str(pat: &str) -> Self {
        MyRegex(Regex::from_str(pat).unwrap())
    }
}

impl Hash for MyRegex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

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
    reaction: HashMap<MyRegex, Box<Callback>>,
    resp: HashMap<MyRegex, Box<Callback>>,
    server: Arc<Mutex<Box<dyn Server>>>,
}

impl Bot {
    pub fn new(config: Config) -> Self {
        // https://rust-unofficial.github.io/patterns/idioms/on-stack-dyn-dispatch.html
        let server: Box<dyn Server> = match config.server {
            ServerType::Shell => Box::new(Shell::new(config.name.clone())),
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
        let re = MyRegex::from_str(pattern);
        self.reaction.entry(re).or_insert_with(|| Box::new(cb));
    }

    pub fn respond(&mut self, pattern: &str, cb: &'static Callback) {
        let pat = format!("{}:? +?{}", self.name, pattern);
        let re = MyRegex::from_str(&pat);
        self.resp.entry(re).or_insert_with(|| Box::new(cb));
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

    pub fn run(&self) {
        let server = self.server.clone();
        let (tx, rx) = channel::<Message>();
        let handle = server.lock().unwrap().connect(tx).unwrap();
        loop {
            let msg = rx.recv().unwrap();
            let message = msg.trim();

            if has_shutdown(&self.name, &message.to_lowercase()) {
                self.shutdown(Some(msg));
                break;
            }

            for (pattern, cb) in &self.resp {
                if pattern.0.is_match(message) {
                    cb(self, &msg.channel, &msg.nick, message);
                }
            }

            for (pattern, cb) in &self.reaction {
                if pattern.0.is_match(message) {
                    cb(self, &msg.channel, &msg.nick, message);
                }
            }
        }

        self.finalize(handle);
    }

    pub fn shutdown(&self, msg: Option<Message>) {
        log::trace!("shutdown");
        if let Some(msg) = msg {
            self.send(&msg.channel, "bye");
        }
        self.server.lock().unwrap().disconnect();
    }

    pub fn finalize(&self, handle: JoinHandle<()>) {
        log::trace!("finalize...");
        handle.join().expect("join fail");
        log::trace!("finalize...done");
    }

    pub fn install_actions(&mut self) {
        // conditional install?
        self.respond("ping", &Action::ping);
    }
}

fn has_shutdown(name: &str, s: &str) -> bool {
    if name.len() >= s.len() || name.ne(&s[0..name.len()]) {
        return false;
    }
    matches!(s[(name.len() + 1)..].trim(), "shutdown")
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_has_shutdown() {
        let s = "hongbot: exit";
        let name = "hongbot";
        assert!(name.eq(&s[0..name.len()]));
        assert_eq!(name, &s[0..name.len()]);
        assert_eq!(&s[(name.len() + 2)..], "exit");
        assert_eq!(s[(name.len() + 1)..].trim(), "exit");
    }
}
