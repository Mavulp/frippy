extern crate unicode_names;

use std::marker::PhantomData;

use irc::client::prelude::*;

use plugin::*;
use FrippyClient;

use error::ErrorKind as FrippyErrorKind;
use error::FrippyError;
use failure::Fail;

#[derive(PluginName, Default, Debug)]
pub struct Unicode<C> {
    phantom: PhantomData<C>,
}

impl<C: FrippyClient> Unicode<C> {
    pub fn new() -> Unicode<C> {
        Unicode {
            phantom: PhantomData,
        }
    }

    fn get_name(&self, symbol: char) -> String {
        match unicode_names::name(symbol) {
            Some(sym) => sym.to_string().to_lowercase(),
            None => String::from("UNKNOWN"),
        }
    }

    fn format_response(&self, content: &str) -> String {
        let character = content.chars().next();
        if let Some(c) = character {
            let mut buf = [0; 4];

            let byte_string = c
                .encode_utf8(&mut buf)
                .as_bytes()
                .iter()
                .map(|b| format!("{:#b}", b))
                .collect::<Vec<String>>()
                .join(",");

            let name = self.get_name(c);

            format!(
                "{} is '{}' | UTF-8: {2:#x} ({2}), Bytes: [{3}]",
                c, name, c as u32, byte_string
            )
        } else {
            String::from("No non-space character was found.")
        }
    }
}

impl<C: FrippyClient> Plugin for Unicode<C> {
    type Client = C;

    fn execute(&self, _: &Self::Client, _: &Message) -> ExecutionStatus {
        ExecutionStatus::Done
    }

    fn execute_threaded(&self, _: &Self::Client, _: &Message) -> Result<(), FrippyError> {
        panic!("Unicode should not use threading")
    }

    fn command(&self, client: &Self::Client, command: PluginCommand) -> Result<(), FrippyError> {
        if command.tokens.is_empty() {
            let msg = "No non-space character was found.";

            if let Err(e) = client.send_notice(command.source, msg) {
                Err(e.context(FrippyErrorKind::Connection))?;
            }

            return Ok(());
        }

        let content = &command.tokens[0];

        if let Err(e) = client.send_privmsg(command.target, &self.format_response(&content)) {
            Err(e.context(FrippyErrorKind::Connection))?;
        }

        Ok(())
    }

    fn evaluate(&self, _: &Self::Client, command: PluginCommand) -> Result<String, String> {
        let tokens = command.tokens;

        if tokens.is_empty() {
            return Err(String::from("No non-space character was found."));
        }

        Ok(self.format_response(&tokens[0]))
    }
}
