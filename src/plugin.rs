//! Definitions required for every `Plugin`
use std::fmt;

use irc::client::prelude::*;
use error::FrippyError;

/// Describes if a [`Plugin`](trait.Plugin.html) is done working on a
/// [`Message`](../../irc/proto/message/struct.Message.html) or if another thread is required.
#[derive(Debug)]
pub enum ExecutionStatus {
    /// The [`Plugin`](trait.Plugin.html) does not need to do any more work on this
    /// [`Message`](../../irc/proto/message/struct.Message.html).
    Done,
    /// An error occured during the execution.
    Err(FrippyError),
    /// The execution needs to be done by [`execute_threaded()`](trait.Plugin.html#tymethod.execute_threaded).
    RequiresThread,
}

/// `Plugin` has to be implemented for any struct that should be usable
/// as a `Plugin` in frippy.
pub trait Plugin: PluginName + Send + Sync + fmt::Debug {
    /// Handles messages which are not commands or returns
    /// [`RequiresThread`](enum.ExecutionStatus.html#variant.RequiresThread)
    /// if [`execute_threaded()`](trait.Plugin.html#tymethod.execute_threaded) should be used instead.
    fn execute(&self, client: &IrcClient, message: &Message) -> ExecutionStatus;
    /// Handles messages which are not commands in a new thread.
    fn execute_threaded(&self, client: &IrcClient, message: &Message) -> Result<(), FrippyError>;
    /// Handles any command directed at this plugin.
    fn command(&self, client: &IrcClient, command: PluginCommand) -> Result<(), FrippyError>;
    /// Similar to [`command()`](trait.Plugin.html#tymethod.command) but return a String instead of
    /// sending messages directly to IRC.
    fn evaluate(&self, client: &IrcClient, command: PluginCommand) -> Result<String, String>;
}

/// `PluginName` is required by [`Plugin`](trait.Plugin.html).
///
/// To implement it simply add `#[derive(PluginName)]`
/// above the definition of the struct.
///
/// # Examples
/// ```ignore
/// #[macro_use] extern crate frippy_derive;
///
/// #[derive(PluginName)]
/// struct Foo;
/// ```
pub trait PluginName: Send + Sync + fmt::Debug {
    /// Returns the name of the `Plugin`.
    fn name(&self) -> &str;
}

/// Represents a command sent by a user to the bot.
#[derive(Clone, Debug)]
pub struct PluginCommand {
    /// The sender of the command.
    pub source: String,
    /// If the command was sent to a channel, this will be that channel
    /// otherwise it is the same as `source`.
    pub target: String,
    /// The remaining part of the message that has not been processed yet - split by spaces.
    pub tokens: Vec<String>,
}

impl PluginCommand {
    /// Creates a `PluginCommand` from [`Message`](../../irc/proto/message/struct.Message.html)
    /// if it contains a [`PRIVMSG`](../../irc/proto/command/enum.Command.html#variant.PRIVMSG)
    /// that starts with the provided `nick`.
    pub fn from(nick: &str, message: &Message) -> Option<PluginCommand> {
        // Get the actual message out of PRIVMSG
        if let Command::PRIVMSG(_, ref content) = message.command {
            // Split content by spaces and filter empty tokens
            let mut tokens: Vec<String> = content.split(' ').map(ToOwned::to_owned).collect();

            // Commands start with our name
            if tokens[0].to_lowercase().starts_with(nick) {
                // Remove the bot's name from the first token
                tokens[0].drain(..nick.len());

                // We assume that only ':' and ',' are used as suffixes on IRC
                // If there are any other chars we assume that it is not ment for the bot
                tokens[0] = tokens[0].chars().filter(|&c| !":,".contains(c)).collect();
                if !tokens[0].is_empty() {
                    return None;
                }

                // The first token contained the name of the bot
                tokens.remove(0);

                Some(PluginCommand {
                    source: message.source_nickname().unwrap().to_string(),
                    target: message.response_target().unwrap().to_string(),
                    tokens: tokens,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}
