use hongbot_rs::bot::{Bot, ServerType};

fn main() {
    // initialize
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info,hongbot=trace"));

    let mut bot = Bot::new("hongbot", ServerType::Shell);
    bot.hear("poop", &|bot, ch, _nick, _msg| {
        bot.send(ch, "oh?");
    });
    bot.respond("ping", &|bot, ch, nick, _msg| {
        bot.reply(ch, nick, "pong");
    });
    bot.run();

    log::trace!("done");
}
