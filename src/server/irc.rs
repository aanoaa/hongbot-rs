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
        if raw.chars().next().unwrap().eq(&':') {
            let v = raw[1..].split(' ').collect::<Vec<&str>>();
            s = v[1..].to_vec();
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
            _ => Err(IrcError::UnknownCommand), // 에러 대신에 ignore 하자
        }?;

        Ok(Self {
            raw: raw.to_string(),
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

        // thread 를 나눠야 할 수도?
        // connection 을 맺고,
        // read 전용
        // callback 이 필요할 듯?
        // join 다음에 할 일 등등

        let handle = thread::spawn(move || {
            // https://www.rfc-editor.org/rfc/rfc1459#section-4.1
            // The recommended order for a client to register is as follows:
            // 1. Pass message
            // 2. Nick message
            // 3. User message
            if let Ok(mut stream) = TcpStream::connect(addr) {
                log::trace!("Connected to the server!");
                {
                    let mut accepted = lock1.write().unwrap();
                    *accepted = true;
                }

                let mut s = stream.try_clone().expect("stream clone fail");
                thread::spawn(move || {
                    let mut buf = [0; 1024];
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
                                            let channel = String::from("irc");
                                            let nick = String::from("you");

                                            // Parameters: <receiver>{,<receiver>} <text to be sent>
                                            // :Angel PRIVMSG Wiz :Hello are you receiving this message ?
                                            //     ; Message from Angel to Wiz.
                                            //
                                            // PRIVMSG Angel :yes I'm receiving it !receiving it !'u>(768u+1n) .br
                                            //     ; Message to Angel.
                                            //
                                            // PRIVMSG jto@tolsun.oulu.fi :Hello !
                                            //     ; Message to a client on server tolsun.oulu.fi with username of "jto".
                                            //
                                            // PRIVMSG $*.fi :Server tolsun.oulu.fi rebooting.
                                            //                                 ; Message to everyone on a server which
                                            //                                 has a name matching *.fi.
                                            //
                                            // PRIVMSG #*.edu :NSFNet is undergoing work, expect interruptions
                                            //                                 ; Message to all users who come from a
                                            //                                 host which has a name matching *.edu.
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

                let dur = Duration::from_millis(1000);
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

                thread::sleep(dur);
                cmd = format!("PRIVMSG aanoaa :Hello{}", CRLF);
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

// def parsemsg(s):
//     """Breaks a message from an IRC server into its prefix, command, and arguments.
//     """
//     prefix = ''
//     trailing = []
//     if not s:
//        raise IRCBadMessage("Empty line.")
//     if s[0] == ':':
//         prefix, s = s[1:].split(' ', 1)
//     if s.find(' :') != -1:
//         s, trailing = s.split(' :', 1)
//         args = s.split()
//         args.append(trailing)
//     else:
//         args = s.split()
//     command = args.pop(0)
//     return prefix, command, args

// parsemsg(":test!~test@test.com PRIVMSG #channel :Hi!")
// # ('test!~test@test.com', 'PRIVMSG', ['#channel', 'Hi!'])

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_irc_message_parse() {
        let msg = IrcMessage::from(":test!~test@test.com PRIVMSG #channel :Hi!").unwrap();
        assert_eq!(msg.raw, ":test!~test@test.com PRIVMSG #channel :Hi!");
        assert_eq!(msg.command, IrcCommand::PRIVMSG);
        assert_eq!(msg.params, "#channel :Hi!");

        let msg = IrcMessage::from("PING; :ynrYzp}[Bx").unwrap();
        assert_eq!(msg.raw, "PING; :ynrYzp}[Bx");
        assert_eq!(msg.command, IrcCommand::PING);
        assert_eq!(msg.params, ":ynrYzp}[Bx");
    }
}
