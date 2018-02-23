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
    fn execute(&self, _: &IrcClient, _: &Message) -> ExecutionStatus {
        ExecutionStatus::Done
    }

    fn execute_threaded(&self, _: &IrcClient, _: &Message) -> Result<(), IrcError> {
        panic!("Help should not use threading")
    }

    fn command(&self, client: &IrcClient, command: PluginCommand) -> Result<(), IrcError> {
        client.send_notice(&command.source, "Help has not been added yet.")
    }

    fn evaluate(&self, _: &IrcClient, _: PluginCommand) -> Result<String, String> {
        Err(String::from("Help has not been added yet."))
    }
}

#[cfg(test)]
mod tests {}
