#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

//! Frippy is an IRC bot that runs plugins on each message
//! received.
//!
//! ## Example
//! ```no_run
//! extern crate frippy;
//!
//! frippy::run();
//! ```
//!
//! # Logging
//! Frippy uses the [log](https://docs.rs/log) crate so you can log events
//! which might be of interest.

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

use std::sync::Arc;

use irc::client::prelude::*;
use irc::error::Error as IrcError;

use tokio_core::reactor::Core;
use futures::future;
use glob::glob;

use plugin::*;

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
            }
            Err(e) => error!("Failed to read path {}", e),
        }
    }

    // Without configs the bot would just idle
    if configs.is_empty() {
        error!("No config file found");
        return;
    }

    // The list of plugins in use
    let mut plugins = ThreadedPlugins::new();
    plugins.add(plugins::Help::new());
    plugins.add(plugins::Emoji::new());
    plugins.add(plugins::Currency::new());
    info!("Plugins loaded: {}", plugins);

    // Create an event loop to run the connections on.
    let mut reactor = Core::new().unwrap();

    // Open a connection and add work for each config
    for config in configs {
        let server =
            match IrcServer::new_future(reactor.handle(), &config).and_then(|f| reactor.run(f)) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to connect: {}", e);
                    return;
                }
            };

        info!("Connected to server");

        match server.identify() {
            Ok(_) => info!("Identified"),
            Err(e) => error!("Failed to identify: {}", e),
        };

        // TODO Verify if we actually need to clone plugins twice
        let plugins = plugins.clone();

        let task = server
            .stream()
            .for_each(move |message| process_msg(&server, plugins.clone(), message))
            .map_err(|e| error!("Failed to process message: {}", e));

        reactor.handle().spawn(task);
    }

    // Run the main loop forever
    reactor.run(future::empty::<(), ()>()).unwrap();
}

fn process_msg(server: &IrcServer,
               mut plugins: ThreadedPlugins,
               message: Message)
               -> Result<(), IrcError> {

    if let Command::JOIN(ref channel, _, _) = message.command {
        if message.source_nickname().unwrap() == server.current_nickname() {
            info!("Joined {}", channel);
        }
    }

    // Check for possible command and save the result for later
    let command = PluginCommand::from(&server.current_nickname().to_lowercase(), &message);

    let message = Arc::new(message);
    plugins.execute_plugins(server, message);

    // If the message contained a command, handle it
    if let Some(command) = command {
        if let Err(e) = plugins.handle_command(server, command) {
            error!("Failed to handle command: {}", e);
        }
    }

    Ok(())
}


#[cfg(test)]
mod tests {}
