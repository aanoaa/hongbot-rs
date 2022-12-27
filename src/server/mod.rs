use std::{sync::mpsc::Sender, thread::JoinHandle};

use anyhow::Result;

use crate::bot::Message;

pub mod irc;
pub mod shell;

pub trait Server {
    /// connect tx is message channel sender that from server to bot
    fn connect(&mut self, tx: Sender<Message>) -> Result<JoinHandle<()>>;
    fn disconnect(&self);
    fn send(&mut self, channel: &str, message: &str);
}
