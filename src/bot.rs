use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    str::FromStr,
    sync::{mpsc::channel, Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
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

type Callback = Box<dyn Fn(&Bot, String, String, String)>;

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
    reaction: HashMap<MyRegex, Callback>,
    resp: HashMap<MyRegex, Callback>,
    pub server: Arc<Mutex<Box<dyn Server + Send>>>,
}

impl Bot {
    pub fn new(config: Config) -> Self {
        // https://rust-unofficial.github.io/patterns/idioms/on-stack-dyn-dispatch.html
        let server: Box<dyn Server + Send> = match config.server {
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

    pub fn hear<F>(&mut self, pattern: &str, cb: F)
    where
        F: Fn(&Bot, String, String, String) + 'static,
    {
        let re = MyRegex::from_str(pattern);
        self.reaction.entry(re).or_insert_with(|| Box::new(cb));
    }

    pub fn respond<F>(&mut self, pattern: &str, cb: F)
    where
        F: Fn(&Bot, String, String, String) + 'static,
    {
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
        // serve http
        let running0 = Arc::new(Mutex::new(true));
        let running1 = running0.clone();
        let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
        listener
            .set_nonblocking(true)
            .expect("cannot set non-blocking");
        let http_serve_handle = thread::spawn(move || {
            log::info!("Listening for connections on port {}", 8080);
            let dur = Duration::from_millis(100);
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        handle_client(stream);
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        if *running0.lock().unwrap() {
                            thread::sleep(dur);
                            continue;
                        } else {
                            drop(listener);
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!("http connect fail: {}", e);
                    }
                }
            }
        });

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
                    cb(
                        self,
                        msg.channel.clone(),
                        msg.nick.clone(),
                        message.to_string(),
                    );
                }
            }

            for (pattern, cb) in &self.reaction {
                if pattern.0.is_match(message) {
                    cb(
                        self,
                        msg.channel.clone(),
                        msg.nick.clone(),
                        message.to_string(),
                    );
                }
            }
        }

        {
            let mut running = running1.lock().unwrap();
            *running = false;
        }
        self.finalize(vec![handle, http_serve_handle]);
    }

    pub fn shutdown(&self, msg: Option<Message>) {
        log::trace!("shutdown");
        if let Some(msg) = msg {
            self.send(&msg.channel, "bye");
        }
        self.server.lock().unwrap().disconnect();
    }

    pub fn finalize(&self, handles: Vec<JoinHandle<()>>) {
        log::trace!("finalize...");
        for handle in handles {
            handle.join().expect("join fail");
        }
        log::trace!("finalize...done");
    }

    pub fn install_actions(&mut self) {
        // conditional install?
        self.respond("ping", Action::ping);
        self.respond("ipaddr", Action::ifconfig);
    }
}

fn has_shutdown(name: &str, s: &str) -> bool {
    if name.len() >= s.len() || name.ne(&s[0..name.len()]) {
        return false;
    }
    matches!(s[(name.len() + 1)..].trim(), "shutdown")
}

fn handle_client(stream: TcpStream) {
    handle_read(&stream);
    handle_write(stream);
}

fn handle_read(mut stream: &TcpStream) {
    let mut buf = [0u8; 4096];
    match stream.read(&mut buf) {
        Ok(_) => {
            let req = String::from_utf8_lossy(&buf);
            log::trace!("{}", req);
        }
        Err(e) => {
            log::error!("http read fail: {e}")
        }
    }
}

fn handle_write(mut stream: TcpStream) {
    const CRLF: &str = "\n\r";
    let resp =
        format!("HTTP/1.1 200 OK{CRLF}Content-Type: text/plain; charset=UTF-8{CRLF}{CRLF}OK{CRLF}");
    match stream.write(resp.as_bytes()) {
        Ok(_) => (),
        Err(e) => {
            log::error!("http resp fail: {e}")
        }
    }
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
