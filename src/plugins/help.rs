use std::marker::PhantomData;

use irc::client::prelude::*;

use plugin::*;
use FrippyClient;

use error::ErrorKind as FrippyErrorKind;
use error::FrippyError;
use failure::ResultExt;

#[derive(PluginName, Default, Debug)]
pub struct Help<C> {
    phantom: PhantomData<C>,
}

impl<C: FrippyClient> Help<C> {
    pub fn new() -> Self {
        Help {
            phantom: PhantomData,
        }
    }
}

impl<C: FrippyClient> Plugin for Help<C> {
    type Client = C;
    fn execute(&self, _: &Self::Client, _: &Message) -> ExecutionStatus {
        ExecutionStatus::Done
    }

    fn execute_threaded(&self, _: &Self::Client, _: &Message) -> Result<(), FrippyError> {
        panic!("Help should not use threading")
    }

    fn command(&self, client: &Self::Client, command: PluginCommand) -> Result<(), FrippyError> {
        client
            .send_notice(
                &command.source,
                "Available commands: help, currency, tell, factoids, remind\r\n\
                 For more detailed help call help on the specific command.\r\n\
                 Example: 'currency help'",
            )
            .context(FrippyErrorKind::Connection)?;

        Ok(())
    }

    fn evaluate(&self, _: &Self::Client, _: PluginCommand) -> Result<String, String> {
        Err(String::from("Help has not been added yet."))
    }
}
