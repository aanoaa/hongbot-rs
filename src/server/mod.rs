use std::{sync::mpsc::Sender, thread::JoinHandle};

use anyhow::Result;

pub mod echo;
pub mod irc;
pub mod shell;

pub trait Server {
    fn connect(&mut self, tx: Sender<(String, String, String)>) -> Result<JoinHandle<()>>;
    fn disconnect(&self);
}
