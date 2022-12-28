use crate::bot::Bot;

pub struct Action {}

impl Action {
    pub fn ping(bot: &Bot, ch: &str, nick: &str, _msg: &str) {
        bot.reply(ch, nick, "pong");
    }
}
