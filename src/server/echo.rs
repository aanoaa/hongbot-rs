use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{mpsc::Sender, Arc, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use super::Server;
use anyhow::Result;

const DEFAULT_ECHO_PORT: i32 = 8080;

#[derive(Debug)]
pub struct Echo {
    accepted: Option<Arc<RwLock<bool>>>,
    port: i32,
    tx: Option<Sender<(String, String, String)>>,
}

impl Echo {
    pub fn new(port: Option<i32>) -> Self {
        let port = port.unwrap_or(DEFAULT_ECHO_PORT);
        Echo {
            accepted: None,
            port,
            tx: None,
        }
    }
}

impl Default for Echo {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Server for Echo {
    fn connect(&mut self, tx0: Sender<(String, String, String)>) -> Result<JoinHandle<()>> {
        let addr0 = format!("127.0.0.1:{}", self.port);
        let addr1 = addr0.clone();

        let lock0 = Arc::new(RwLock::new(false));
        let lock1 = Arc::clone(&lock0);

        self.accepted = Some(lock0);
        self.tx = Some(tx0.clone());

        thread::spawn(move || {
            log::trace!("{addr0} echo server started");
            let listener = TcpListener::bind(addr0).unwrap();
            while let Ok((stream, addr)) = listener.accept() {
                log::trace!("connection accepted: {:?}", addr);
                thread::spawn(move || {
                    handle_connection(stream).unwrap();
                });
            }
        });

        Ok(thread::spawn(move || {
            // wait for echo server ready
            let dur = Duration::from_millis(10);
            thread::sleep(dur);

            if let Ok(mut stream) = TcpStream::connect(addr1) {
                log::trace!("Connected to the server!");
                {
                    let mut accepted = lock1.write().unwrap();
                    *accepted = true;
                }

                let stdin = io::stdin();
                let mut stdout = io::stdout();
                let mut buf = String::new();
                loop {
                    let channel = String::from("echo");
                    let nick = String::from("you");
                    print!("{:>7}#{}> ", nick, channel);
                    stdout.flush().unwrap();
                    stdin.read_line(&mut buf).expect("read fail");
                    stream.write_all(buf.as_bytes()).unwrap();
                    let message = buf.trim();
                    tx0.send((channel, nick, message.to_string()))
                        .expect("send fail");

                    buf.clear();
                    thread::sleep(dur);

                    let accepted = lock1.read().unwrap();
                    if !*accepted {
                        stream
                            .shutdown(std::net::Shutdown::Both)
                            .expect("shutdown fail");
                        break;
                    }
                }
            } else {
                log::error!("Couldn't connect to server...");
            }
        }))
    }

    fn disconnect(&self) {
        log::trace!("disconnect");
        if let Some(lock) = &self.accepted {
            let mut lock = lock.write().expect("acquire write lock fail");
            *lock = false;
        }

        // lock 으로는 client 에서 disconnect
        // 타이밍 맞춰서 client 끊어지면 echo server 에 대한 shutdown
        // + join thread
    }

    fn send(&mut self, channel: &str, message: &str) {}
}

fn handle_connection(mut stream: TcpStream) -> Result<()> {
    let mut buf = [0; 4096];
    while match stream.read(&mut buf) {
        Ok(size) => {
            stream.write_all(&buf[0..size]).unwrap();
            true
        }
        Err(e) => {
            log::error!("read fail: {e}");
            stream
                .shutdown(std::net::Shutdown::Both)
                .expect("shutdown fail");
            false
        }
    } {}
    Ok(())
}
