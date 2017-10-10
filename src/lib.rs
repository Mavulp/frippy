#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

//! Frippy is an IRC bot that runs plugins on each message
//! received.
//!
//! # Example
//! ```no_run
//! extern crate frippy;
//!
//! frippy::run();
//! ```

#[macro_use]
extern crate lazy_static;

extern crate irc;
extern crate regex;

#[macro_use]
mod plugin;
mod plugins;

use std::thread::spawn;
use std::sync::{Arc, Mutex};
use regex::Regex;
use irc::client::prelude::*;
use irc::proto::Command::PRIVMSG;
use irc::error::Error as IrcError;

use plugin::Plugin;
use plugin::PluginCommand;

/// Runs the bot
///
/// # Remarks
///
/// This blocks the current thread while the bot is running
pub fn run() {
    let server = IrcServer::new("config.toml").unwrap();
    server.identify().unwrap();

    // The list of plugins in use
    let plugins: Vec<Arc<Mutex<Plugin>>> =
        vec![Arc::new(Mutex::new(plugins::emoji::Emoji::new())),
             Arc::new(Mutex::new(plugins::currency::Currency::new()))];

    // We need the plugins' names to make sure the user gets a response
    // if they use an incorrect plugin name
    let plugin_names: Vec<String> = plugins
        .iter()
        .map(|p| p.lock().unwrap().to_string().to_lowercase())
        .collect();

    // The main loop over received messages
    server
        .for_each_incoming(|message| {
            let message = Arc::new(message);
            // Check for possible command and save the result for later
            let command = get_command(&server.current_nickname().to_lowercase(), &message);

            // Check if the first token of the command is valid
            if let Some(ref c) = command {
                if c.tokens.is_empty() {
                    let help = format!("Use \"{} help\" to get help", server.current_nickname());
                    server.send_notice(&c.source, &help).unwrap();

                } else if "help" == &c.tokens[0].to_lowercase() {
                    send_help_message(&server, c).unwrap();

                } else if !plugin_names.contains(&c.tokens[0].to_lowercase()) {

                    let help = format!("\"{} {}\" is not a command, \
                                       try \"{0} help\" instead.",
                                       server.current_nickname(),
                                       c.tokens[0]);

                    server.send_notice(&c.source, &help).unwrap();
                }
            }

            for plugin in plugins.clone() {
                // Clone everything before the move
                let server = server.clone();
                let message = Arc::clone(&message);
                let command = command.clone();

                // Spawn a new thread for each plugin
                spawn(move || {
                    // Lock the mutex and ignore if it is poisoned
                    let mut plugin = match plugin.lock() {
                        Ok(plugin) => plugin,
                        Err(poisoned) => poisoned.into_inner(),
                    };

                    // Send the message to the plugin if the plugin needs it
                    if plugin.is_allowed(&server, &message) {
                        plugin.execute(&server, &message).unwrap();
                    }

                    // Check if the command is for this plugin
                    if let Some(mut c) = command {
                        if !c.tokens.is_empty() &&
                           plugin.to_string().to_lowercase() == c.tokens[0].to_lowercase() {

                            // The first token contains the name of the plugin
                            c.tokens.remove(0);
                            plugin.command(&server, c).unwrap();
                        }
                    }
                });
            }
        })
        .unwrap();
}

fn send_help_message(server: &IrcServer, command: &PluginCommand) -> Result<(), IrcError> {
    server.send_notice(&command.source, "Help has not been added yet.")
}

fn get_command(nick: &str, message: &Message) -> Option<PluginCommand> {

    // Get the actual message out of PRIVMSG
    if let PRIVMSG(_, ref content) = message.command {

        // Split content by spaces and filter empty tokens
        let mut tokens: Vec<String> = content
            .split(' ')
            .filter(|&x| !x.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        // Check if the message contained notthing but spaces
        if tokens.is_empty() {
            return None;
        }

        // Only compile the regex once
        // We assume that only ':' and ',' are used as suffixes on IRC
        lazy_static! {
            static ref RE: Regex = Regex::new("^[:,]*?$").unwrap();
        }

        if tokens[0].to_lowercase().starts_with(nick) {

            // Remove the bot's name from the first token
            tokens[0].drain(..nick.len());

            // If the regex does not match the message is not directed at the bot
            if !RE.is_match(&tokens[0]) {
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

#[cfg(test)]
mod tests {}
