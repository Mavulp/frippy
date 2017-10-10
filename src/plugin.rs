use std::fmt;
use irc::client::prelude::*;
use irc::error::Error as IrcError;

pub trait Plugin: PluginName + Send + Sync + fmt::Debug {
    fn is_allowed(&self, server: &IrcServer, message: &Message) -> bool;
    fn execute(&mut self, server: &IrcServer, message: &Message) -> Result<(), IrcError>;
    fn command(&mut self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError>;
}

pub trait PluginName: Send + Sync + fmt::Debug {
    fn name(&self) -> &str;
}

#[derive(Clone, Debug)]
pub struct PluginCommand {
    pub source: String,
    pub target: String,
    pub tokens: Vec<String>,
}
