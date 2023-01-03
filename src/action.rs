use std::{thread, time::Duration};

use crate::bot::Bot;

pub struct Action {}

impl Action {
    pub fn ping(bot: &Bot, ch: String, nick: String, _msg: String) {
        bot.reply(&ch, &nick, "pong");
    }

    pub fn ping_with_delayed_pong(bot: &Bot, ch: String, _nick: String, _msg: String) {
        let serv = bot.server.clone();
        thread::spawn(move || {
            // a long task here
            thread::sleep(Duration::from_secs(1));
            serv.lock().unwrap().send(&ch, "pong");
        });
    }
}
