use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::bot::Bot;

pub struct Action {}

impl Action {
    pub fn ping(bot: &Bot, ch: &str, nick: &str, _msg: &str) {
        bot.reply(ch, nick, "pong");
    }

    pub fn ping_with_delayed_pong(bot: &Bot, ch: &str, _nick: &str, _msg: &str) {
        let s0 = Arc::new(Mutex::new(String::new()));
        let s1 = s0.clone();
        let handle = thread::spawn(move || {
            // a long task here
            thread::sleep(Duration::from_secs(5));
            s0.lock().unwrap().push_str("pong after 5s");
        });

        handle.join().expect("join thread handle fail");
        bot.send(ch, s1.lock().unwrap().as_str());
    }
}
