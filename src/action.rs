use std::{thread, time::Duration};

use curl::easy::Easy;
use regex::Captures;

use crate::bot::Bot;

pub struct Action {}

impl Action {
    pub fn ping(bot: &Bot, ch: String, nick: String, _msg: String, _caps: Captures) {
        bot.reply(&ch, &nick, "pong");
    }

    pub fn ping_delayed(bot: &Bot, ch: String, _nick: String, _msg: String, _caps: Captures) {
        let serv = bot.server.clone();
        thread::spawn(move || {
            // a long task here
            thread::sleep(Duration::from_secs(1));
            serv.lock().unwrap().send(&ch, "pong");
        });
    }

    pub fn ifconfig(bot: &Bot, ch: String, _nick: String, _msg: String, _caps: Captures) {
        let serv = bot.server.clone();
        thread::spawn(move || {
            let mut easy = Easy::new();
            easy.url("https://ifconfig.me/").unwrap();
            easy.write_function(move |data| {
                let s = std::str::from_utf8(data).unwrap();
                serv.lock().unwrap().send(&ch, s);
                Ok(data.len())
            })
            .unwrap();
            easy.perform().unwrap();
        });
    }
}
