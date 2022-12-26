use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{mpsc::Sender, Arc, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Result;
use thiserror::Error;

use super::Server;

const CRLF: &str = "\r\n";

#[derive(Debug)]
pub struct Irc {
    pub nick: String,
    pub server: String,
    accepted: Option<Arc<RwLock<bool>>>,
}

#[derive(Debug, PartialEq)]
enum IrcCommand {
    PING,
    PONG,
    USER,
    PRIVMSG,
    JOIN,
}

#[derive(Debug)]
struct IrcMessage {
    raw: String,
    nick: Option<String>,
    servername: Option<String>,
    hostname: Option<String>,
    command: IrcCommand,
    params: String,
}

#[derive(Debug, Error)]
enum IrcError {
    #[error("unknown command")]
    UnknownCommand,
    #[error("invalid message")]
    InvalidMessage,
}

impl IrcMessage {
    pub fn from(raw: &str) -> Result<Self> {
        if raw.is_empty() {
            return Err(IrcError::InvalidMessage.into());
        }

        let mut s: Vec<&str>;
        let mut nick = None;
        let mut servername = None;
        let mut hostname = None;
        if raw.chars().next().unwrap().eq(&':') {
            let v = raw[1..].split(' ').collect::<Vec<&str>>();
            s = v[1..].to_vec();
            let from = v[0];
            if let Some(i) = from.find('!') {
                nick = Some(from[0..i].to_string());
                if let Some(j) = from.find('@') {
                    servername = Some(from[(i + 1)..j].to_string());
                    hostname = Some(from[(j + 1)..].to_string());
                };
            };
        } else {
            s = raw.split(' ').collect::<Vec<&str>>();
        }

        let cmd = s[0].replace(';', "");
        s = s[1..].to_vec();

        let command = match cmd.to_lowercase().as_str() {
            "ping" => Ok(IrcCommand::PING),
            "pong" => Ok(IrcCommand::PONG),
            "user" => Ok(IrcCommand::USER),
            "privmsg" => Ok(IrcCommand::PRIVMSG),
            "join" => Ok(IrcCommand::JOIN),
            _ => Err(IrcError::UnknownCommand),
        }?;

        Ok(Self {
            raw: raw.to_string(),
            nick,
            servername,
            hostname,
            command,
            params: s.join(" "),
        })
    }
}

impl Irc {
    pub fn new(nick: &str, server: &str) -> Self {
        Irc {
            nick: nick.to_string(),
            server: server.to_string(),
            accepted: None,
        }
    }
}

impl Default for Irc {
    fn default() -> Self {
        Self::new("hongbot", "localhost:6667")
    }
}

impl Server for Irc {
    fn connect(&mut self, tx: Sender<(String, String, String)>) -> Result<JoinHandle<()>> {
        let nick = self.nick.clone();
        let addr = self.server.clone();

        let lock0 = Arc::new(RwLock::new(false));
        let lock1 = Arc::clone(&lock0);
        self.accepted = Some(lock0);
        let handle = thread::spawn(move || {
            // https://www.rfc-editor.org/rfc/rfc1459#section-4.1
            // The recommended order for a client to register is as follows:
            // 1. Pass message
            // 2. Nick message
            // 3. User message
            // + response ping message
            if let Ok(mut stream) = TcpStream::connect(addr) {
                log::trace!("Connected to the server!");
                {
                    let mut accepted = lock1.write().unwrap();
                    *accepted = true;
                }

                let mut s = stream.try_clone().expect("stream clone fail");
                thread::spawn(move || {
                    let mut buf = [0; 4096];
                    loop {
                        match s.read(&mut buf) {
                            Ok(size) => {
                                if size == 0 {
                                    continue;
                                }
                                log::trace!("read {} bytes", size);
                                let message = std::str::from_utf8(&buf[0..size])
                                    .expect("unexpected string bytes");
                                let message = String::from(message);
                                log::trace!("{}", message);
                                buf[0..size].iter_mut().for_each(|x| *x = 0);

                                // ignore unknown commands
                                let irc_msg = IrcMessage::from(&message).ok();
                                if let Some(msg) = irc_msg {
                                    match msg.command {
                                        IrcCommand::PING => {
                                            let command = format!("PONG {}", msg.params);
                                            log::trace!("{:?}", command);
                                            s.write_all(command.as_bytes()).unwrap();
                                        }
                                        IrcCommand::PRIVMSG => {
                                            let nick = if let Some(nick) = msg.nick {
                                                nick
                                            } else {
                                                "".to_string()
                                            };

                                            let s = msg.params.split(' ').collect::<Vec<&str>>();
                                            if s.len() < 2 {
                                                // ignore
                                                log::error!("unexpected message");
                                                continue;
                                            }

                                            let channel = String::from(s[0]);
                                            let message = s[1..].join(" ");
                                            tx.send((channel, nick, message.trim().to_string()))
                                                .expect("send fail");
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("read fail: {e}");
                                s.shutdown(std::net::Shutdown::Both).unwrap();
                                break;
                            }
                        }

                        let accepted = lock1.read().unwrap();
                        if !*accepted {
                            s.shutdown(std::net::Shutdown::Both).expect("shutdown fail");
                            break;
                        }
                    }
                });

                let dur = Duration::from_millis(5000);
                thread::sleep(dur);

                let mut cmd = format!("NICK {}{}", nick, CRLF);
                match stream.write_all(cmd.as_bytes()) {
                    Ok(()) => {
                        log::trace!("wrote {:?}", &cmd);
                    }
                    Err(e) => {
                        //
                        log::error!("wrote {:?} fail: {e}", &cmd);
                    }
                }

                thread::sleep(dur);

                // Parameters: <username> <hostname> <servername> <realname>
                // USER guest tolmoon tolsun :Ronnie Reagan
                //                                 ; User registering themselves with a
                //                                 username of "guest" and real name
                //                                 "Ronnie Reagan".

                // :testnick USER guest tolmoon tolsun :Ronnie Reagan
                //                                 ; message between servers with the
                //                                 nickname for which the USER command
                //                                 belongs to

                // configuration 으로 부터 compose
                cmd = format!("USER {} * * :hongbot-rs{}", nick, CRLF);
                match stream.write_all(cmd.as_bytes()) {
                    Ok(()) => {
                        log::trace!("wrote {:?}", &cmd);
                    }
                    Err(e) => {
                        log::error!("wrote {:?} fail: {e}", &cmd);
                    }
                }

                thread::sleep(dur);

                cmd = format!("JOIN #foo{}", CRLF);
                match stream.write_all(cmd.as_bytes()) {
                    Ok(()) => {
                        log::trace!("wrote {:?}", &cmd);
                    }
                    Err(e) => {
                        log::error!("wrote {:?} fail: {e}", &cmd);
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_irc_message_parse() {
        let msg = IrcMessage::from(":test!~test@test.com PRIVMSG #channel :Hi!").unwrap();
        assert_eq!(msg.raw, ":test!~test@test.com PRIVMSG #channel :Hi!");
        assert_eq!(msg.command, IrcCommand::PRIVMSG);
        assert_eq!(msg.params, "#channel :Hi!");
        assert_eq!(msg.nick, Some("test".to_string()));
        assert_eq!(msg.servername, Some("~test".to_string()));
        assert_eq!(msg.hostname, Some("test.com".to_string()));

        // ping message
        let msg = IrcMessage::from("PING; :ynrYzp}[Bx").unwrap();
        assert_eq!(msg.raw, "PING; :ynrYzp}[Bx");
        assert_eq!(msg.command, IrcCommand::PING);
        assert_eq!(msg.params, ":ynrYzp}[Bx");
        assert_eq!(msg.nick, None);
        assert_eq!(msg.servername, None);
        assert_eq!(msg.hostname, None);

        // direct message from aanoaa -> hongbot
        let msg = IrcMessage::from(":aanoaa!user@172.21.0.1 PRIVMSG hongbot :bye").unwrap();
        assert_eq!(msg.raw, ":aanoaa!user@172.21.0.1 PRIVMSG hongbot :bye");
        assert_eq!(msg.command, IrcCommand::PRIVMSG);
        assert_eq!(msg.params, "hongbot :bye");
        assert_eq!(msg.nick, Some("aanoaa".to_string()));
        assert_eq!(msg.servername, Some("user".to_string()));
        assert_eq!(msg.hostname, Some("172.21.0.1".to_string()));

        // #foo channel message
        let msg = IrcMessage::from(":aanoaa!user@172.21.0.1 PRIVMSG #foo :good").unwrap();
        assert_eq!(msg.raw, ":aanoaa!user@172.21.0.1 PRIVMSG #foo :good");
        assert_eq!(msg.command, IrcCommand::PRIVMSG);
        assert_eq!(msg.params, "#foo :good");
        assert_eq!(msg.nick, Some("aanoaa".to_string()));
        assert_eq!(msg.servername, Some("user".to_string()));
        assert_eq!(msg.hostname, Some("172.21.0.1".to_string()));
    }
}
