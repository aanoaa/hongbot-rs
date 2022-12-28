use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{mpsc::Sender, Arc, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Result;
use thiserror::Error;

use crate::{bot::Message, config::IrcConfig};

use super::Server;

const CRLF: &str = "\r\n";

#[derive(Debug)]
pub struct Irc {
    config: IrcConfig,
    accepted: Option<Arc<RwLock<bool>>>,
    stream: Option<TcpStream>,
}

#[derive(Debug, PartialEq)]
enum IrcCommand {
    Ping,
    Pong,
    User,
    Privmsg,
    Join,
}

#[allow(dead_code)]
#[derive(Debug)]
struct IrcMessage {
    raw: String,
    servername: Option<String>,
    hostname: Option<String>,
    nick: Option<String>,
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
            "ping" => Ok(IrcCommand::Ping),
            "pong" => Ok(IrcCommand::Pong),
            "user" => Ok(IrcCommand::User),
            "privmsg" => Ok(IrcCommand::Privmsg),
            "join" => Ok(IrcCommand::Join),
            _ => Err(IrcError::UnknownCommand),
        }?;

        Ok(Self {
            raw: raw.to_string(),
            servername,
            hostname,
            nick,
            command,
            params: s.join(" "),
        })
    }
}

impl Irc {
    pub fn new(config: IrcConfig) -> Self {
        Irc {
            config,
            accepted: None,
            stream: None,
        }
    }
}

impl Server for Irc {
    fn connect(&mut self, tx: Sender<Message>) -> Result<JoinHandle<()>> {
        // 1. pass (optional)
        // 2. nick
        // 3. user
        // 4. pong (resp ping)
        let nick = self.config.nick.clone();
        let addr = self.config.addr.clone();

        let lock0 = Arc::new(RwLock::new(false));
        let lock1 = Arc::clone(&lock0);
        self.accepted = Some(lock0);

        let mut stream0 = TcpStream::connect(addr)?;
        self.stream = Some(stream0.try_clone().unwrap());
        log::trace!("Connected to the server!");
        {
            let mut accepted = lock1.write().unwrap();
            *accepted = true;
        }

        let pass = self.config.pass.clone();
        let channels = self.config.channels.clone();
        let user = if let Some(user) = self.config.user.as_ref() {
            user.clone()
        } else {
            nick.clone()
        };
        let realname = if let Some(user) = self.config.realname.as_ref() {
            user.clone()
        } else {
            nick.clone()
        };
        let mut stream1 = stream0.try_clone().expect("stream clone fail");
        let handle = thread::spawn(move || {
            let mut buf = [0; 4096];
            loop {
                match stream1.read(&mut buf) {
                    Ok(size) => {
                        let message =
                            std::str::from_utf8(&buf[0..size]).expect("unexpected string bytes");
                        let message = String::from(message);
                        // cleanup buffer
                        buf[0..size].iter_mut().for_each(|x| *x = 0);

                        // ignore unknown commands
                        let irc_msg = IrcMessage::from(&message).ok();
                        if let Some(msg) = irc_msg {
                            match msg.command {
                                IrcCommand::Ping => {
                                    handle_ping(&mut stream1, msg);
                                }
                                IrcCommand::Privmsg => {
                                    handle_privmsg(&tx, msg);
                                }
                                _ => {
                                    log::trace!("{:?}", msg);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("read fail: {e}");
                        stream1.shutdown(std::net::Shutdown::Both).unwrap();
                        break;
                    }
                }

                let accepted = lock1.read().unwrap();
                if !*accepted {
                    stream1
                        .shutdown(std::net::Shutdown::Both)
                        .expect("shutdown fail");
                    break;
                }
            }
        });

        let sec = Duration::from_millis(1000);
        if let Some(pass) = pass {
            stream0
                .write_all(format!("PASS {}{CRLF}", pass).as_bytes())
                .expect("send PASS cmd fail");
            thread::sleep(sec * 3);
        }

        stream0
            .write_all(format!("NICK {}{CRLF}", nick).as_bytes())
            .expect("send NICK cmd fail");
        thread::sleep(sec * 3);

        // Parameters: <username> <hostname> <servername> <realname>
        //
        // USER guest tolmoon tolsun :Ronnie Reagan
        // ; User registering themselves with a username of "guest" and real name "Ronnie Reagan".
        //
        // :testnick USER guest tolmoon tolsun :Ronnie Reagan
        // ; message between servers with the nickname for which the USER command belongs to
        stream0
            .write_all(format!("USER {} * * :{}{CRLF}", user, realname).as_bytes())
            .expect("send USER cmd fail");
        thread::sleep(sec * 3);

        for ch in &channels {
            stream0
                .write_all(format!("JOIN {}{CRLF}", ch).as_bytes())
                .expect("send JOIN cmd fail");
            thread::sleep(sec);
        }

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
        if let Some(stream) = &mut self.stream {
            let command = format!("PRIVMSG {} {}{}", channel, message, CRLF);
            match stream.write_all(command.as_bytes()) {
                Ok(()) => {
                    log::trace!("wrote {:?}", &command);
                }
                Err(e) => {
                    log::error!("wrote {:?} fail: {e}", &command);
                }
            }
        }
    }
}

fn handle_ping(stream: &mut TcpStream, msg: IrcMessage) {
    let command = format!("PONG {}{CRLF}", msg.params);
    stream.write_all(command.as_bytes()).unwrap();
}

fn handle_privmsg(tx: &Sender<Message>, msg: IrcMessage) {
    let nick = msg.nick.unwrap_or_else(|| "unknown".to_string());
    let params = msg.params.split(' ').collect::<Vec<&str>>();
    if params.len() < 2 {
        log::error!("unexpected privmsg format: {:?}", msg.params);
        return;
    }
    let channel = String::from(params[0]);
    let message = params[1..].join(" ");
    tx.send(Message {
        channel,
        nick,
        message,
    })
    .expect("tx send fail");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_irc_message_parse() {
        let msg = IrcMessage::from(":test!~test@test.com PRIVMSG #channel :Hi!").unwrap();
        assert_eq!(msg.raw, ":test!~test@test.com PRIVMSG #channel :Hi!");
        assert_eq!(msg.servername, Some("~test".to_string()));
        assert_eq!(msg.hostname, Some("test.com".to_string()));
        assert_eq!(msg.command, IrcCommand::Privmsg);
        assert_eq!(msg.params, "#channel :Hi!");
        assert_eq!(msg.nick, Some("test".to_string()));

        // ping message
        let msg = IrcMessage::from("PING; :ynrYzp}[Bx").unwrap();
        assert_eq!(msg.raw, "PING; :ynrYzp}[Bx");
        assert_eq!(msg.servername, None);
        assert_eq!(msg.hostname, None);
        assert_eq!(msg.command, IrcCommand::Ping);
        assert_eq!(msg.params, ":ynrYzp}[Bx");
        assert_eq!(msg.nick, None);

        // direct message from aanoaa -> hongbot
        let msg = IrcMessage::from(":aanoaa!user@172.21.0.1 PRIVMSG hongbot :bye").unwrap();
        assert_eq!(msg.raw, ":aanoaa!user@172.21.0.1 PRIVMSG hongbot :bye");
        assert_eq!(msg.servername, Some("user".to_string()));
        assert_eq!(msg.hostname, Some("172.21.0.1".to_string()));
        assert_eq!(msg.command, IrcCommand::Privmsg);
        assert_eq!(msg.params, "hongbot :bye");
        assert_eq!(msg.nick, Some("aanoaa".to_string()));

        // #foo channel message
        let msg = IrcMessage::from(":aanoaa!user@172.21.0.1 PRIVMSG #foo :good").unwrap();
        assert_eq!(msg.raw, ":aanoaa!user@172.21.0.1 PRIVMSG #foo :good");
        assert_eq!(msg.servername, Some("user".to_string()));
        assert_eq!(msg.hostname, Some("172.21.0.1".to_string()));
        assert_eq!(msg.command, IrcCommand::Privmsg);
        assert_eq!(msg.params, "#foo :good");
        assert_eq!(msg.nick, Some("aanoaa".to_string()));
    }
}
