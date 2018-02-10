use irc::client::prelude::*;
use irc::error::IrcError;

use plugin::*;

#[derive(PluginName, Default, Debug)]
pub struct Help;

impl Help {
    pub fn new() -> Help {
        Help {}
    }
}

impl Plugin for Help {
    fn is_allowed(&self, _: &IrcClient, _: &Message) -> bool {
        false
    }

    fn execute(&self, _: &IrcClient, _: &Message) -> Result<(), IrcError> {
        panic!("Help does not implement the execute function!")
    }

    fn command(&self, server: &IrcClient, command: PluginCommand) -> Result<(), IrcError> {
        server.send_notice(&command.source, "Help has not been added yet.")
    }

    fn evaluate(&self, _: &IrcClient, _: PluginCommand) -> Result<String, String> {
        Err(String::from("Help has not been added yet."))
    }
}

#[cfg(test)]
mod tests {}
