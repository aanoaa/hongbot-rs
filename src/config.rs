use std::path::Path;

use config::ConfigError;
use serde::Deserialize;

use crate::bot::ServerType;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub name: String,
    pub server: ServerType,
    pub scripts: Vec<String>,
    pub irc: Option<IrcConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IrcConfig {
    pub nick: String,
    pub user: Option<String>,
    pub pass: Option<String>,
    pub realname: Option<String>,
    pub addr: String,
    pub channels: Vec<String>,
}

impl Config {
    pub fn from(path: &Path) -> Result<Self, ConfigError> {
        config::Config::builder()
            .add_source(config::File::from(path))
            .build()?
            .try_deserialize()
    }
}
