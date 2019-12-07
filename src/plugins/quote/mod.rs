use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

use antidote::RwLock;
use chrono::NaiveDateTime;
use irc::client::prelude::*;
use rand::{thread_rng, Rng};
use time;

use crate::plugin::*;
use crate::FrippyClient;
pub mod database;
use self::database::Database;

use self::error::*;
use crate::error::ErrorKind as FrippyErrorKind;
use crate::error::FrippyError;
use failure::ResultExt;

enum QuoteResponse {
    Public(String),
    Private(String),
}

#[derive(PluginName)]
pub struct Quote<T: Database, C: Client> {
    quotes: RwLock<T>,
    phantom: PhantomData<C>,
}

impl<T: Database, C: Client> Quote<T, C> {
    pub fn new(db: T) -> Self {
        Quote {
            quotes: RwLock::new(db),
            phantom: PhantomData,
        }
    }

    fn create_quote(
        &self,
        quotee: &str,
        channel: &str,
        content: &str,
        author: &str,
    ) -> Result<&str, QuoteError> {
        let count = self.quotes.read().count_user_quotes(quotee, channel)?;
        let tm = time::now().to_timespec();

        let quote = database::NewQuote {
            quotee,
            channel,
            idx: count + 1,
            content,
            author,
            created: NaiveDateTime::from_timestamp(tm.sec, 0u32),
        };

        Ok(self
            .quotes
            .write()
            .insert_quote(&quote)
            .map(|()| "Successfully added!")?)
    }

    fn add(&self, command: &mut PluginCommand) -> Result<&str, QuoteError> {
        if command.tokens.len() < 2 {
            Err(ErrorKind::InvalidCommand)?;
        }

        if command.target == command.source {
            Err(ErrorKind::PrivateMessageNotAllowed)?;
        }

        let quotee = command.tokens.remove(0);
        let channel = &command.target;
        let content = command.tokens.join(" ");

        Ok(self.create_quote(&quotee, channel, &content, &command.source)?)
    }

    fn get(&self, command: &PluginCommand) -> Result<String, QuoteError> {
        let quotee = &command.tokens.get(0);
        let channel = &command.target;
        let count = if let Some(quotee) = quotee {
            self.quotes.read().count_user_quotes(quotee, channel)?
        } else {
            self.quotes.read().count_channel_quotes(channel)?
        };

        if count < 1 {
            Err(ErrorKind::NotFound)?;
        }

        let len = command.tokens.len();
        let idx = if len < 2 || command.tokens[1].is_empty() {
            thread_rng().gen_range(1, count + 1)
        } else {
            let idx_string = &command.tokens[1];
            let idx = i32::from_str(idx_string).context(ErrorKind::InvalidIndex)?;

            if idx < 0 {
                count + idx + 1
            } else {
                idx
            }
        };

        let response = if let Some(quotee) = quotee {
            let quote = self
                .quotes
                .read()
                .get_user_quote(quotee, channel, idx)
                .context(ErrorKind::NotFound)?;

            format!(
                "\"{}\" - {}[{}/{}]",
                quote.content, quote.quotee, quote.idx, count
            )
        } else {
            let quote = self
                .quotes
                .read()
                .get_channel_quote(channel, idx)
                .context(ErrorKind::NotFound)?;

            format!(
                "\"{}\" - {}[{}]",
                quote.content, quote.quotee, quote.idx
            )
        };

        Ok(response)
    }

    fn info(&self, command: &PluginCommand) -> Result<String, QuoteError> {
        let tokens = command
            .tokens
            .iter()
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>();
        match tokens.len() {
            0 => {
                let channel = &command.target;
                let count = self.quotes.read().count_channel_quotes(channel)?;

                Ok(match count {
                    0 => Err(ErrorKind::NotFound)?,
                    1 => format!("1 quote was saved in {}", channel),
                    _ => format!("{} quotes were saved in {}", count, channel),
                })
            }
            1 => {
                let quotee = &command.tokens[0];
                let channel = &command.target;
                let count = self.quotes.read().count_user_quotes(quotee, channel)?;

                Ok(match count {
                    0 => Err(ErrorKind::NotFound)?,
                    1 => format!("{} has 1 quote", quotee),
                    _ => format!("{} has {} quotes", quotee, count),
                })
            }
            _ => {
                let quotee = &command.tokens[0];
                let channel = &command.target;
                let idx = i32::from_str(&command.tokens[1]).context(ErrorKind::InvalidIndex)?;

                let idx = if idx < 0 {
                    self.quotes.read().count_user_quotes(quotee, channel)? + idx + 1
                } else {
                    idx
                };

                let quote = self
                    .quotes
                    .read()
                    .get_user_quote(quotee, channel, idx)
                    .context(ErrorKind::NotFound)?;

                Ok(format!(
                    "{}'s quote was added by {} at {} UTC",
                    quotee, quote.author, quote.created
                ))
            }
        }
    }

    fn help(&self) -> &str {
        "usage: quotes <subcommand>\r\n\
         subcommands: add, get, info, help"
    }
}

impl<T: Database, C: FrippyClient> Plugin for Quote<T, C> {
    type Client = C;
    fn execute(&self, _: &Self::Client, _: &Message) -> ExecutionStatus {
        ExecutionStatus::Done
    }

    fn execute_threaded(&self, _: &Self::Client, _: &Message) -> Result<(), FrippyError> {
        panic!("Quotes should not use threading")
    }

    fn command(
        &self,
        client: &Self::Client,
        mut command: PluginCommand,
    ) -> Result<(), FrippyError> {
        use self::QuoteResponse::{Private, Public};

        if command.tokens.is_empty() {
            client
                .send_notice(&command.source, &ErrorKind::InvalidCommand.to_string())
                .context(FrippyErrorKind::Connection)?;

            return Ok(());
        }

        let target = command.target.clone();
        let source = command.source.clone();

        let sub_command = command.tokens.remove(0);
        let result = match sub_command.as_ref() {
            "add" => self.add(&mut command).map(|s| Private(s.to_owned())),
            "get" => self.get(&command).map(Public),
            "info" => self.info(&command).map(Public),
            "help" => Ok(Private(self.help().to_owned())),
            _ => Err(ErrorKind::InvalidCommand.into()),
        };

        match result {
            Ok(v) => match v {
                Public(m) => client
                    .send_privmsg(&target, &m)
                    .context(FrippyErrorKind::Connection)?,
                Private(m) => client
                    .send_notice(&source, &m)
                    .context(FrippyErrorKind::Connection)?,
            },
            Err(e) => {
                let message = e.to_string();
                client
                    .send_notice(&source, &message)
                    .context(FrippyErrorKind::Connection)?;
                Err(e).context(FrippyErrorKind::Quote)?
            }
        }

        Ok(())
    }

    fn evaluate(&self, _: &Self::Client, _: PluginCommand) -> Result<String, String> {
        Err(String::from(
            "Evaluation of commands is not implemented for Quote at this time",
        ))
    }
}

impl<T: Database, C: FrippyClient> fmt::Debug for Quote<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Quote {{ ... }}")
    }
}

pub mod error {
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Fail, Error)]
    #[error = "QuoteError"]
    pub enum ErrorKind {
        /// Invalid command error
        #[fail(display = "Incorrect command. Send \"quote help\" for help")]
        InvalidCommand,

        /// Invalid index error
        #[fail(display = "Invalid index")]
        InvalidIndex,

        /// Private message error
        #[fail(display = "You can only add quotes in channel messages")]
        PrivateMessageNotAllowed,

        /// Download  error
        #[fail(display = "Download failed")]
        Download,

        /// Duplicate error
        #[fail(display = "Entry already exists")]
        Duplicate,

        /// Not found error
        #[fail(display = "Quote was not found")]
        NotFound,

        /// MySQL error
        #[cfg(feature = "mysql")]
        #[fail(display = "Failed to execute MySQL Query")]
        MysqlError,

        /// No connection error
        #[cfg(feature = "mysql")]
        #[fail(display = "No connection to the database")]
        NoConnection,
    }
}
