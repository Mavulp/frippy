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
extern crate log;
#[macro_use]
extern crate plugin_derive;

extern crate irc;
extern crate tokio_core;
extern crate futures;
extern crate glob;

mod plugin;
mod plugins;

use std::thread::spawn;
use std::sync::{Arc, Mutex};
use glob::glob;

use irc::client::prelude::*;
use irc::proto::Command::PRIVMSG;
use irc::error::Error as IrcError;

use tokio_core::reactor::Core;
use futures::future;

use plugin::*;

// Lock the mutex and ignore if it is poisoned
macro_rules! lock_plugin {
    ($e:expr) => {
        match $e.lock() {
            Ok(plugin) => plugin,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// Runs the bot
///
/// # Remarks
///
/// This blocks the current thread while the bot is running
pub fn run() {

    // Load all toml files in the configs directory
    let mut configs = Vec::new();
    for toml in glob("configs/*.toml").unwrap() {
        match toml {
            Ok(path) => {
                info!("Loading {}", path.to_str().unwrap());
                match Config::load(path) {
                    Ok(v) => configs.push(v),
                    Err(e) => error!("Incorrect config file {}", e),
                }
            },
            Err(e) => error!("Failed to read path {}", e),
        }
    }

    // Without configs the bot would just idle
    if configs.is_empty() {
        error!("No config file found");
        return;
    }

    // The list of plugins in use
    let plugins: Vec<Arc<Mutex<Plugin>>> =
        vec![Arc::new(Mutex::new(plugins::emoji::Emoji::new())),
             Arc::new(Mutex::new(plugins::currency::Currency::new()))];

    // We need the plugins' names to make sure the user gets a response
    // if they use an incorrect plugin name
    let plugin_names: Vec<String> = plugins
        .iter()
        .map(|p| lock_plugin!(p).name().to_lowercase())
        .collect();

    info!("Plugins loaded: {}", plugin_names.join(", "));

    // Create an event loop to run the connections on.
    let mut reactor = Core::new().unwrap();

    // Open a connection and add work for each config
    for config in configs {
        let fut = IrcServer::new_future(reactor.handle(), &config).unwrap();

        let server = match reactor.run(fut) {
            Ok(v) => {
                info!("Connected to server");
                v
            }
            Err(e) => {
                error!("Failed to connect: {}", e);
                return;
            }
        };

        match server.identify() {
            Ok(_) => info!("Intentified"),
            Err(e) => error!("Failed to identify: {}", e),
        };

        // TODO Duplicate clone...
        let plugins = plugins.clone();
        let plugin_names = plugin_names.clone();

        let task = server
            .stream()
            .for_each(move |message| process_msg(&server, &plugin_names, plugins.clone(), message))
            .map_err(|e| Err(e).unwrap());

        reactor.handle().spawn(task);
    }

    // Run the main loop forever
    reactor.run(future::empty::<(), ()>()).unwrap();
}

fn process_msg(server: &IrcServer,
               plugin_names: &[String],
               plugins: Vec<Arc<Mutex<Plugin>>>,
               message: Message)
               -> Result<(), IrcError> {

    if let Command::JOIN(ref channel, _, _) = message.command {
        info!("Joined {}", channel);
    }

    let message = Arc::new(message);
    // Check for possible command and save the result for later
    let command = get_command(&server.current_nickname().to_lowercase(), &message);

    // Check if the first token of the command is valid
    if let Some(ref c) = command {
        if c.tokens.is_empty() {
            let help = format!("Use \"{} help\" to get help", server.current_nickname());
            server.send_notice(&c.source, &help).unwrap();

        } else if "help" == &c.tokens[0].to_lowercase() {
            send_help_message(server, c).unwrap();

        } else if !plugin_names.contains(&c.tokens[0].to_lowercase()) {

            let help = format!("\"{} {}\" is not a command, \
                                try \"{0} help\" instead.",
                               server.current_nickname(),
                               c.tokens[0]);

            server.send_notice(&c.source, &help).unwrap();
        }
    }

    for plugin in plugins {
        // Send the message to the plugin if the plugin needs it
        if lock_plugin!(plugin).is_allowed(server, &message) {

            // Clone everything before the move
            // The server uses an Arc internally too
            let plugin = Arc::clone(&plugin);
            let message = Arc::clone(&message);
            let server = server.clone();

            // Execute the plugin in another thread
            spawn(move || { lock_plugin!(plugin).execute(&server, &message).unwrap(); });
        }

        // Check if the command is for this plugin
        if let Some(mut c) = command.clone() {

            // Skip empty commands
            if c.tokens.is_empty() {
                continue;
            }

            if lock_plugin!(plugin).name().to_lowercase() == c.tokens[0].to_lowercase() {

                // The first token contains the name of the plugin
                let name = c.tokens.remove(0);

                // Clone the server for the move - it uses an Arc internally
                let server = server.clone();
                spawn(move || {
                          if let Err(e) = lock_plugin!(plugin).command(&server, c) {
                              error!("Error in {} - {}", name, e);
                          };
                      });
            }
        }
    }
    Ok(())
}

fn send_help_message(server: &IrcServer, command: &PluginCommand) -> Result<(), IrcError> {
    server.send_notice(&command.source, "Help has not been added yet.")
}

fn get_command(nick: &str, message: &Message) -> Option<PluginCommand> {

    // Get the actual message out of PRIVMSG
    if let PRIVMSG(_, ref content) = message.command {

        // Split content by spaces and filter empty tokens
        let mut tokens: Vec<String> = content.split(' ').map(ToOwned::to_owned).collect();

        // Commands start with our name
        if tokens[0].to_lowercase().starts_with(nick) {

            // Remove the bot's name from the first token
            tokens[0].drain(..nick.len());

            // We assume that only ':' and ',' are used as suffixes on IRC
            // If there are any other chars we assume that it is not ment for the bot
            tokens[0] = tokens[0]
                .chars()
                .filter(|&c| !":,".contains(c))
                .collect();
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

#[cfg(test)]
mod tests {}
