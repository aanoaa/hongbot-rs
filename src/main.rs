use hongbot_rs::bot::Bot;

fn main() {
    // initialize
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info,hongbot=trace"));

    let mut bot = Bot::new("hongbot");
    bot.run();

    log::trace!("done");
}
