use std::fmt;
use std::marker::PhantomData;

use antidote::RwLock;
use irc::client::prelude::*;

use crate::plugin::*;
use crate::FrippyClient;
pub mod database;
use self::database::Database;

use self::error::*;
use crate::error::ErrorKind as FrippyErrorKind;
use crate::error::FrippyError;
use failure::ResultExt;

use frippy_derive::PluginName;

#[derive(PluginName)]
pub struct Counter<T: Database, C: Client> {
    counts: RwLock<T>,
    phantom: PhantomData<C>,
}

impl<T: Database, C: Client> Counter<T, C> {
    pub fn new(db: T) -> Self {
        Self {
            counts: RwLock::new(db),
            phantom: PhantomData,
        }
    }

    fn get(&self, name: &str) -> Result<String, CounterError> {
        self.counts.read().get_count(name).map(|c| c.to_string())
    }

    fn add(&self, name: &str) -> Result<String, CounterError> {
        self.counts.write().add(name).map(|c| c.to_string())
    }

    fn subtract(&self, name: &str) -> Result<String, CounterError> {
        self.counts.write().subtract(name).map(|c| c.to_string())
    }
}

impl<T: Database, C: FrippyClient> Plugin for Counter<T, C> {
    type Client = C;
    fn execute(&self, _: &Self::Client, message: &Message) -> ExecutionStatus {
        if let Command::PRIVMSG(_, content) = message.command.clone() {
            if content.contains(' ') || !content.is_char_boundary(content.len() - 2) {
                return ExecutionStatus::Done;
            }
            if ["++", "--", "=="].contains(&&content[content.len() - 2..]) {
                return ExecutionStatus::RequiresThread;
            }
        }

        ExecutionStatus::Done
    }

    fn execute_threaded(
        &self,
        client: &Self::Client,
        message: &Message,
    ) -> Result<(), FrippyError> {
        if let Command::PRIVMSG(_, content) = message.command.clone() {
            let (name, end) = content.split_at(content.len() - 2);
            let count = match end {
                "++" => self.add(name),
                "--" => self.subtract(name),
                "==" => self.get(name),
                _ => unreachable!("execute checks this already"),
            }
            .context(FrippyErrorKind::Counter)?;

            client
                .send_privmsg(message.response_target().unwrap_or(""), count)
                .context(FrippyErrorKind::Connection)?;
        }

        Ok(())
    }

    fn command(&self, client: &Self::Client, command: PluginCommand) -> Result<(), FrippyError> {
        client
            .send_privmsg(
                command.target,
                "This Plugin does not implement any commands.",
            )
            .context(FrippyErrorKind::Connection)?;

        Ok(())
    }

    fn evaluate(&self, _: &Self::Client, _: PluginCommand) -> Result<String, String> {
        Err(String::from(
            "Evaluation of commands is not implemented for Factoid at this time",
        ))
    }
}

impl<T: Database, C: FrippyClient> fmt::Debug for Counter<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Counter {{ ... }}")
    }
}

pub mod error {
    use failure::Fail;
    use frippy_derive::Error;

    #[derive(Copy, Clone, Eq, PartialEq, Debug, Fail, Error)]
    #[error = "CounterError"]
    pub enum ErrorKind {
        /// MySQL error
        #[fail(display = "Failed to execute MySQL Query")]
        MysqlError,

        /// No connection error
        #[fail(display = "No connection to the database")]
        NoConnection,
    }
}
