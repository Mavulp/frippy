use irc::client::prelude::*;
use irc::error::Error as IrcError;

use plugin::*;

#[derive(PluginName, Debug)]
pub struct Help;

impl Help {
    pub fn new() -> Help {
        Help {}
    }

    fn help(&self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError> {
        server.send_notice(&command.source, "Help has not been added yet.")
    }
}

impl Plugin for Help {
    fn is_allowed(&self, _: &IrcServer, _: &Message) -> bool {
        false
    }

    fn execute(&mut self, _: &IrcServer, _: &Message) -> Result<(), IrcError> {
        panic!("Help does not implement the execute function!")
    }

    fn command(&mut self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError> {
        self.help(server, command)
    }
}

#[cfg(test)]
mod tests {}
