//! Definitions required for every `Plugin`
use std::fmt;

use crate::error::FrippyError;
use irc::client::prelude::*;

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
    type Client;
    /// Handles messages which are not commands or returns
    /// [`RequiresThread`](enum.ExecutionStatus.html#variant.RequiresThread)
    /// if [`execute_threaded()`](trait.Plugin.html#tymethod.execute_threaded) should be used instead.
    fn execute(&self, client: &Self::Client, message: &Message) -> ExecutionStatus;
    /// Handles messages which are not commands in a new thread.
    fn execute_threaded(&self, client: &Self::Client, message: &Message)
        -> Result<(), FrippyError>;
    /// Handles any command directed at this plugin.
    fn command(&self, client: &Self::Client, command: PluginCommand) -> Result<(), FrippyError>;
    /// Similar to [`command()`](trait.Plugin.html#tymethod.command) but return a String instead of
    /// sending messages directly to IRC.
    fn evaluate(&self, client: &Self::Client, command: PluginCommand) -> Result<String, String>;
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
pub trait PluginName {
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
    pub fn try_from(prefix: &str, message: &Message) -> Option<PluginCommand> {
        // Get the actual message out of PRIVMSG
        if let Command::PRIVMSG(_, ref content) = message.command {
            // Split content by spaces
            let mut tokens: Vec<String> = content.split(' ').map(ToOwned::to_owned).collect();

            // Commands start with a prefix
            if !tokens[0].to_lowercase().starts_with(prefix) {
                return None;
            }
            // Remove the prefix from the first token
            tokens[0].drain(..prefix.len());

            Some(PluginCommand {
                source: message.source_nickname().unwrap().to_string(),
                target: message.response_target().unwrap().to_string(),
                tokens,
            })
        } else {
            None
        }
    }
}
