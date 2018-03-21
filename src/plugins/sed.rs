use std::collections::HashMap;
use circular_queue::CircularQueue;
use regex::{Regex, RegexBuilder};
use antidote::RwLock;

use irc::client::prelude::*;

use plugin::*;

use failure::Fail;
use failure::ResultExt;
use error::ErrorKind as FrippyErrorKind;
use error::FrippyError;
use self::error::*;

lazy_static! {
    static ref RE: Regex = Regex::new(r"^s/((?:\\/|[^/])+)/((?:\\/|[^/])*)/(?:(\w+))?\s*$").unwrap();
}

#[derive(PluginName, Debug)]
pub struct Sed {
    per_channel: usize,
    channel_messages: RwLock<HashMap<String, CircularQueue<String>>>,
}

impl Sed {
    pub fn new(per_channel: usize) -> Sed {
        Sed {
            per_channel: per_channel,
            channel_messages: RwLock::new(HashMap::new()),
        }
    }

    fn add_message(&self, channel: String, message: String) {
        let mut channel_messages = self.channel_messages.write();
        let messages = channel_messages
            .entry(channel)
            .or_insert(CircularQueue::with_capacity(self.per_channel));
        messages.push(message);
    }

    fn format_escaped(&self, input: &str) -> String {
        let mut output = String::with_capacity(input.len());
        let mut escape = false;

        for c in input.chars() {
            if escape && !r"/\".contains(c) {
                output.push('\\');
            } else if !escape && c == '\\' {
                escape = true;
                continue;
            }
            escape = false;

            output.push(c);
        }

        output
    }

    fn run_regex(&self, channel: &str, message: &str) -> Result<String, SedError> {
        let mut global_match = false;
        let mut case_insens = false;
        let mut ign_whitespace = false;
        let mut swap_greed = false;
        let mut disable_unicode = false;

        let captures = RE.captures(message).unwrap();
        debug!("{:?}", captures);

        let first = self.format_escaped(captures.get(1).unwrap().as_str());
        let second = self.format_escaped(captures.get(2).unwrap().as_str());

        if let Some(flags) = captures.get(3) {
            let flags = flags.as_str();

            global_match = flags.contains('g');
            case_insens = flags.contains('i');
            ign_whitespace = flags.contains('x');
            swap_greed = flags.contains('U');
            disable_unicode = !flags.contains('u');
        }

        let user_re = RegexBuilder::new(&first)
            .case_insensitive(case_insens)
            .ignore_whitespace(ign_whitespace)
            .unicode(disable_unicode)
            .swap_greed(swap_greed)
            .build()
            .context(ErrorKind::InvalidRegex)?;

        let channel_messages = self.channel_messages.read();
        let messages = channel_messages.get(channel).ok_or(ErrorKind::NoMessages)?;

        for message in messages.iter() {
            if user_re.is_match(message) {
                let response = if global_match {
                    user_re.replace_all(message, &second[..])
                } else {
                    user_re.replace(message, &second[..])
                };

                return Ok(response.to_string());
            }
        }

        Err(ErrorKind::NoMatch)?
    }
}

impl Plugin for Sed {
    fn execute(&self, client: &IrcClient, message: &Message) -> ExecutionStatus {
        match message.command {
            Command::PRIVMSG(_, ref content) => {
                let channel = message.response_target().unwrap();
                if channel == message.source_nickname().unwrap() {
                    return ExecutionStatus::Done;
                }

                if RE.is_match(content) {
                    let result = match self.run_regex(channel, content) {
                        Ok(msg) => client.send_privmsg(channel, &msg),
                        Err(e) => client.send_notice(channel, &e.to_string()),
                    };

                    match result {
                        Err(e) => {
                            ExecutionStatus::Err(e.context(FrippyErrorKind::Connection).into())
                        }
                        Ok(_) => ExecutionStatus::Done,
                    }
                } else {
                    self.add_message(channel.to_string(), content.to_string());

                    ExecutionStatus::Done
                }
            }
            _ => ExecutionStatus::Done,
        }
    }

    fn execute_threaded(&self, _: &IrcClient, _: &Message) -> Result<(), FrippyError> {
        panic!("Sed should not use threading")
    }

    fn command(&self, client: &IrcClient, command: PluginCommand) -> Result<(), FrippyError> {
        Ok(client
            .send_notice(
                &command.source,
                "Currently this Plugin does not implement any commands.",
            )
            .context(FrippyErrorKind::Connection)?)
    }

    fn evaluate(&self, _: &IrcClient, _: PluginCommand) -> Result<String, String> {
        Err(String::from(
            "Evaluation of commands is not implemented for sed at this time",
        ))
    }
}

pub mod error {
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Fail, Error)]
    #[error = "SedError"]
    pub enum ErrorKind {
        /// Invalid regex error
        #[fail(display = "Invalid regex")]
        InvalidRegex,

        /// No messages found error
        #[fail(display = "No messages were found for this channel")]
        NoMessages,

        /// No match found error
        #[fail(display = "No recent messages match this regex")]
        NoMatch,
    }
}
