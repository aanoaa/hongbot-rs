use std::path::PathBuf;

use hongbot_rs::{
    bot::{Bot, ServerType},
    config::Config,
};

fn main() {
    // initialize
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let config = Config::from(&PathBuf::from("config.toml")).expect("read config fail");
    log::trace!("{:#?}", config);

    let mut bot = Bot::new("hongbot", ServerType::Shell);
    bot.run();
}
