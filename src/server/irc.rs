use std::{
    sync::mpsc::Sender,
    thread::{self, JoinHandle},
};

use anyhow::Result;

use super::Server;

#[derive(Debug)]
pub struct Irc {
    pub nick: String,
    pub server: String,
}

impl Irc {
    pub fn new(nick: &str, server: &str) -> Self {
        Irc {
            nick: nick.to_string(),
            server: server.to_string(),
        }
    }
}

impl Default for Irc {
    fn default() -> Self {
        Self::new("hongbot", "irc.silex.kr:6697")
    }
}

impl Server for Irc {
    fn connect(&mut self, _tx: Sender<(String, String, String)>) -> Result<JoinHandle<()>> {
        // let server = self.server.clone();
        let handle = thread::spawn(move || {
            // let mut stream = TcpStream::connect(server).unwrap();
            // https://www.rfc-editor.org/rfc/rfc1459#section-4.1
            // The recommended order for a client to register is as follows:
            // 1. Pass message
            // 2. Nick message
            // 3. User message
            // 연속한 메세지 보내놓고 메세지 받으면서 parse 하고 주요 이벤트 발생하면 수행
        });

        Ok(handle)
    }

    fn disconnect(&self) {}
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
