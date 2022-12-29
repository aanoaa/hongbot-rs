use std::path::PathBuf;

use hongbot_rs::{bot::Bot, config::Config};

fn main() {
    // initialize
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let config = Config::from(&PathBuf::from("config.toml")).expect("read config fail");
    log::trace!("{:#?}", config);

    let mut bot = Bot::new(config);
    bot.install_actions();
    bot.run();
}
