use irc::client::prelude::*;

use plugin::*;

use error::FrippyError;
use error::ErrorKind as FrippyErrorKind;
use failure::ResultExt;

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

    fn execute_threaded(&self, _: &IrcClient, _: &Message) -> Result<(), FrippyError> {
        panic!("Help should not use threading")
    }

    fn command(&self, client: &IrcClient, command: PluginCommand) -> Result<(), FrippyError> {
        Ok(client
            .send_notice(&command.source, "Help has not been added yet.")
            .context(FrippyErrorKind::Connection)?)
    }

    fn evaluate(&self, _: &IrcClient, _: PluginCommand) -> Result<String, String> {
        Err(String::from("Help has not been added yet."))
    }
}
